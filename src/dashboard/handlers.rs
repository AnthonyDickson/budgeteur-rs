//! Dashboard HTTP handlers and view rendering.
//!
//! This module contains:
//! - Route handlers for displaying and updating the dashboard
//! - HTML view functions for rendering the dashboard UI
//! - State and form types used by the handlers

use axum::{
    extract::{FromRef, State},
    response::{IntoResponse, Response},
};
use axum_extra::extract::Form;
use maud::{Markup, html};
use rusqlite::Connection;
use serde::Deserialize;
use std::{
    collections::HashSet,
    ops::RangeInclusive,
    sync::{Arc, Mutex},
};
use time::{Date, Duration, OffsetDateTime, UtcOffset};

use crate::{
    AppState, Error,
    account::get_total_account_balance,
    dashboard::{
        charts::{DashboardChart, balances_chart, charts_script, expenses_chart, net_income_chart},
        preferences::{get_excluded_tags, save_excluded_tags},
        tables::{monthly_summary_table, summary_statistics_table},
        transaction::{Transaction, get_transactions_in_date_range},
    },
    database_id::DatabaseId,
    endpoints,
    html::{HeadElement, base, link},
    navigation::NavBar,
    tag::{Tag, get_all_tags},
    timezone::get_local_offset,
};

/// Number of days to look back for yearly summary calculations  
const YEARLY_PERIOD_DAYS: i64 = 365;

/// The state needed for displaying the dashboard page.
///
/// Contains the database connection and timezone information required
/// by dashboard handlers.
#[derive(Debug, Clone)]
pub struct DashboardState {
    /// The database connection for managing transactions.
    pub db_connection: Arc<Mutex<Connection>>,
    /// The local timezone as a canonical timezone name, e.g. "Pacific/Auckland".
    pub local_timezone: String,
}

impl FromRef<AppState> for DashboardState {
    fn from_ref(state: &AppState) -> Self {
        Self {
            db_connection: state.db_connection.clone(),
            local_timezone: state.local_timezone.clone(),
        }
    }
}

/// Form data for updating excluded tags.
#[derive(Deserialize)]
pub struct ExcludedTagsForm {
    /// List of tag IDs to exclude from dashboard summaries
    #[serde(default)]
    pub excluded_tags: Vec<DatabaseId>,
}

/// A tag paired with its exclusion status for the dashboard filter UI.
///
/// Used to render checkboxes that allow users to exclude specific tags
/// from dashboard calculations.
#[derive(Debug, Clone)]
struct TagWithExclusion {
    /// The tag
    tag: Tag,
    /// Whether this tag is currently excluded from dashboard summaries
    is_excluded: bool,
}

/// Holds all the data needed to render the dashboard.
struct DashboardData {
    tags_with_status: Vec<TagWithExclusion>,
    charts: [DashboardChart; 3],
    tables: Vec<Markup>,
}

/// Display a page with an overview of the user's data.
pub async fn get_dashboard_page(State(state): State<DashboardState>) -> Result<Response, Error> {
    let connection = state
        .db_connection
        .lock()
        .inspect_err(|error| tracing::error!("could not acquire database lock: {error}"))
        .map_err(|_| Error::DatabaseLockError)?;

    let nav_bar = NavBar::new(endpoints::DASHBOARD_VIEW);

    // Get excluded tags from database
    let excluded_tag_ids = get_excluded_tags(&connection)
        .inspect_err(|error| tracing::error!("could not get excluded tags: {error}"))?;

    // Build all dashboard data
    match build_dashboard_data(&excluded_tag_ids, &state.local_timezone, &connection)? {
        Some(data) => {
            Ok(
                dashboard_view(nav_bar, &data.tags_with_status, &data.charts, &data.tables)
                    .into_response(),
            )
        }
        None => Ok(dashboard_no_data_view(nav_bar).into_response()),
    }
}

/// API endpoint to update excluded tags and return updated summaries
pub async fn update_excluded_tags(
    State(state): State<DashboardState>,
    Form(form): Form<ExcludedTagsForm>,
) -> Response {
    let connection = match state.db_connection.lock() {
        Ok(connection) => connection,
        Err(error) => {
            tracing::error!("could not acquire database lock: {error}");
            return Error::DatabaseLockError.into_alert_response();
        }
    };

    // Save the excluded tags to database
    if let Err(error) = save_excluded_tags(&form.excluded_tags, &connection) {
        tracing::error!("Failed to save dashboard preferences: {error}");
        return Error::DashboardPreferencesSaveError.into_alert_response();
    }

    // Build all dashboard data with the new exclusions
    let data = match build_dashboard_data(&form.excluded_tags, &state.local_timezone, &connection) {
        Ok(Some(data)) => data,
        Ok(None) => {
            // Shouldn't happen since we're updating filters, not deleting all transactions
            tracing::warn!("No transaction data after updating excluded tags");
            return Error::DatabaseLockError.into_alert_response();
        }
        Err(error) => {
            tracing::error!("Failed to build dashboard data: {error}");
            return error.into_alert_response();
        }
    };

    dashboard_content_partial(&data.tags_with_status, &data.charts, &data.tables).into_response()
}

/// Gets the date range for dashboard queries (last year from today).
///
/// # Arguments
/// * `local_timezone` - The local timezone offset
///
/// # Returns
/// Inclusive date range from one year ago to today.
fn get_dashboard_date_range(local_timezone: UtcOffset) -> RangeInclusive<Date> {
    let today = OffsetDateTime::now_utc().to_offset(local_timezone).date();
    let one_year_ago = today - Duration::days(YEARLY_PERIOD_DAYS);
    one_year_ago..=today
}

/// Fetches and builds all data needed for the dashboard display.
///
/// # Arguments
/// * `excluded_tag_ids` - Tag IDs to exclude from calculations
/// * `local_timezone_name` - Timezone name like "Pacific/Auckland"
/// * `connection` - Database connection
///
/// # Returns
/// All dashboard data ready for rendering, or `None` if no transaction data exists.
///
/// # Errors
/// Returns error if database queries fail or timezone is invalid.
fn build_dashboard_data(
    excluded_tag_ids: &[DatabaseId],
    local_timezone_name: &str,
    connection: &Connection,
) -> Result<Option<DashboardData>, Error> {
    // Get all tags and build tags with exclusion status
    let available_tags = get_all_tags(connection)
        .inspect_err(|error| tracing::error!("could not get tags: {error}"))?;

    let excluded_set: HashSet<_> = excluded_tag_ids.iter().collect();
    let tags_with_status: Vec<TagWithExclusion> = available_tags
        .into_iter()
        .map(|tag| TagWithExclusion {
            is_excluded: excluded_set.contains(&tag.id),
            tag,
        })
        .collect();

    // Get timezone offset
    let local_timezone = get_local_offset(local_timezone_name).ok_or_else(|| {
        tracing::error!("Invalid timezone {}", local_timezone_name);
        Error::InvalidTimezoneError(local_timezone_name.to_owned())
    })?;

    // Prepare excluded tags for query
    let excluded_tags_slice = if excluded_tag_ids.is_empty() {
        None
    } else {
        Some(excluded_tag_ids)
    };

    // Get transactions for the last year
    let date_range = get_dashboard_date_range(local_timezone);
    let transactions = get_transactions_in_date_range(date_range, excluded_tags_slice, connection)
        .inspect_err(|error| {
            tracing::error!("Could not get transactions for last year: {error}")
        })?;

    // Return None if no transaction data exists
    if transactions.is_empty() {
        return Ok(None);
    }

    // Get total account balance
    let total_account_balance = get_total_account_balance(connection).inspect_err(|error| {
        tracing::error!("Could not calculate total account balance: {error}")
    })?;

    // Build charts and tables
    let charts = build_dashboard_charts(&transactions, total_account_balance);
    let tables = vec![
        summary_statistics_table(&transactions, total_account_balance),
        monthly_summary_table(&transactions, total_account_balance),
    ];

    Ok(Some(DashboardData {
        tags_with_status,
        charts,
        tables,
    }))
}

/// Creates the array of dashboard charts from transaction data.
///
/// Generates three charts: net income, balances, and expenses by tag.
/// The chart options are serialized to JSON for ECharts consumption.
///
/// # Arguments
/// * `transactions` - Transaction data for the last year
/// * `total_account_balance` - Current total balance across all accounts
///
/// # Returns
/// Array of three DashboardChart instances ready for rendering.
fn build_dashboard_charts(
    transactions: &[Transaction],
    total_account_balance: f64,
) -> [DashboardChart; 3] {
    [
        DashboardChart {
            id: "net-income-chart",
            options: net_income_chart(transactions).to_string(),
        },
        DashboardChart {
            id: "balances-chart",
            options: balances_chart(total_account_balance, transactions).to_string(),
        },
        DashboardChart {
            id: "expenses-chart",
            options: expenses_chart(transactions).to_string(),
        },
    ]
}

/// Renders the dashboard page when no transaction data exists.
///
/// Displays a helpful message with links to add transactions manually
/// or via import.
///
/// # Arguments
/// * `nav_bar` - Navigation bar component
fn dashboard_no_data_view(nav_bar: NavBar) -> Markup {
    let nav_bar = nav_bar.into_html();
    let new_transaction_link = link(endpoints::NEW_TRANSACTION_VIEW, "manually");
    let import_transaction_link = link(endpoints::IMPORT_VIEW, "importing");

    let content = html!(
        (nav_bar)

        div class="flex flex-col items-center px-6 py-8 mx-auto text-gray-900 dark:text-white"
        {
            h2 class="text-xl font-bold"
            {
                "Nothing here yet..."
            }

            p
            {
                "Charts will show up here once you add some transactions.
                You can add transactions " (new_transaction_link) " or
                by " (import_transaction_link) "."
            }
        }
    );

    base("Dashboard", &[], &content)
}

/// Renders the main dashboard page with charts, tables, and tag filter controls.
///
/// # Arguments
/// * `nav_bar` - Navigation bar component
/// * `tags_with_status` - Tags with their exclusion status for the filter UI
/// * `charts` - Dashboard charts to display
/// * `tables` - Dashboard tables to display
fn dashboard_view<'a>(
    nav_bar: NavBar<'a>,
    tags_with_status: &[TagWithExclusion],
    charts: &[DashboardChart],
    tables: &[Markup],
) -> Markup {
    let nav_bar = nav_bar.into_html();
    let excluded_tags_endpoint = endpoints::DASHBOARD_EXCLUDED_TAGS;

    let content = html!(
        (nav_bar)

        div
            id="dashboard-content"
            class="flex flex-col items-center px-2 lg:px-6 lg:py-8 mx-auto
                max-w-screen-xl text-gray-900 dark:text-white"
        {
            section
                id="charts"
                class="w-full mx-auto mb-4"
            {
                div class="grid grid-cols-1 xl:grid-cols-2 gap-4"
                {
                    @for chart in charts {
                        div
                            id=(chart.id)
                            class="min-h-[380px] rounded dark:bg-gray-100"
                        {}
                    }

                    @for table in tables {
                        (table)
                    }
                }
            }

            @if !tags_with_status.is_empty() {
                div class="mb-8 w-full"
                {
                    h3 class="text-xl font-semibold mb-4" { "Filter Out Tags" }

                    form
                        hx-post=(excluded_tags_endpoint)
                        hx-target="#dashboard-content"
                        hx-target-error="#alert-container"
                        hx-swap="innerHTML"
                        hx-trigger="change"
                        class="bg-gray-50 dark:bg-gray-800 p-4 rounded-lg"
                    {
                        p class="text-sm text-gray-600 dark:text-gray-400 mb-3"
                        {
                            "Exclude transactions with these tags from the charts and table above:"
                        }

                        div class="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-4 gap-3"
                        {
                            @for tag_status in tags_with_status {
                                label class="flex items-center space-x-2"
                                {
                                    input
                                        type="checkbox"
                                        name="excluded_tags"
                                        value=(tag_status.tag.id)
                                        checked[tag_status.is_excluded]
                                        class="rounded-sm border-gray-300
                                            text-blue-600 shadow-xs
                                            focus:border-blue-300 focus:ring-3
                                            focus:ring-blue-200/50"
                                    ;

                                    span
                                        class="inline-flex items-center
                                            px-2.5 py-0.5
                                            text-xs font-semibold text-blue-800
                                            bg-blue-100 rounded-full
                                            dark:bg-blue-900 dark:text-blue-300"
                                    {
                                        (tag_status.tag.name)
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    );

    let scripts = [
        HeadElement::ScriptLink("/static/echarts.6.0.0.min.js".to_owned()),
        HeadElement::ScriptLink("/static/echarts-gl.2.0.9.min.js".to_owned()),
        charts_script(charts),
    ];

    base("Dashboard", &scripts, &content)
}

/// Renders the updated dashboard content (charts and tables) for HTMX updates.
///
/// This is used when the tag filter is changed to update the dashboard
/// without requiring a full page reload.
///
/// # Arguments
/// * `tags_with_status` - Tags with their exclusion status for the filter UI
/// * `charts` - Dashboard charts to display
/// * `tables` - Dashboard tables to display
fn dashboard_content_partial(
    tags_with_status: &[TagWithExclusion],
    charts: &[DashboardChart],
    tables: &[Markup],
) -> Markup {
    let excluded_tags_endpoint = endpoints::DASHBOARD_EXCLUDED_TAGS;

    html!(
        section
            id="charts"
            class="w-full mx-auto mb-4"
        {
            div class="grid grid-cols-1 xl:grid-cols-2 gap-4"
            {
                @for chart in charts {
                    div
                        id=(chart.id)
                        class="min-h-[380px] rounded dark:bg-gray-100"
                    {}
                }

                @for table in tables {
                    (table)
                }
            }
        }

        @if !tags_with_status.is_empty() {
            div class="mb-8 w-full"
            {
                h3 class="text-xl font-semibold mb-4" { "Filter Out Tags" }

                form
                    hx-post=(excluded_tags_endpoint)
                    hx-target="#dashboard-content"
                    hx-target-error="#alert-container"
                    hx-swap="innerHTML"
                    hx-trigger="change"
                    class="bg-gray-50 dark:bg-gray-800 p-4 rounded-lg"
                {
                    p class="text-sm text-gray-600 dark:text-gray-400 mb-3"
                    {
                        "Exclude transactions with these tags from the charts and table above:"
                    }

                    div class="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-4 gap-3"
                    {
                        @for tag_status in tags_with_status {
                            label class="flex items-center space-x-2"
                            {
                                input
                                    type="checkbox"
                                    name="excluded_tags"
                                    value=(tag_status.tag.id)
                                    checked[tag_status.is_excluded]
                                    class="rounded-sm border-gray-300
                                        text-blue-600 shadow-xs
                                        focus:border-blue-300 focus:ring-3
                                        focus:ring-blue-200/50"
                                ;

                                span
                                    class="inline-flex items-center
                                        px-2.5 py-0.5
                                        text-xs font-semibold text-blue-800
                                        bg-blue-100 rounded-full
                                        dark:bg-blue-900 dark:text-blue-300"
                                {
                                    (tag_status.tag.name)
                                }
                            }
                        }
                    }
                }
            }
        }
    )
}

#[cfg(test)]
mod tests {
    use axum::{
        body::Body,
        extract::State,
        http::{Response, StatusCode},
    };
    use scraper::{Html, Selector};
    use time::{Duration, OffsetDateTime};

    use crate::{
        dashboard::handlers::DashboardState,
        db::initialize,
        tag::{TagName, create_tag},
        transaction::{Transaction, create_transaction},
    };

    use rusqlite::Connection;
    use std::sync::{Arc, Mutex};

    use super::{ExcludedTagsForm, get_dashboard_page};

    fn get_test_connection() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();
        conn
    }

    #[tokio::test]
    async fn dashboard_page_loads_successfully() {
        let conn = get_test_connection();
        let today = OffsetDateTime::now_utc().date();

        // Create some test data
        create_transaction(Transaction::build(100.0, today, ""), &conn).unwrap();
        create_transaction(
            Transaction::build(-50.0, today - Duration::days(15), ""),
            &conn,
        )
        .unwrap();

        let state = DashboardState {
            db_connection: Arc::new(Mutex::new(conn)),
            local_timezone: "Etc/UTC".to_owned(),
        };

        let response = get_dashboard_page(State(state)).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let html = parse_html(response).await;
        assert_valid_html(&html);

        // Check that charts are present
        assert_chart_exists(&html, "net-income-chart");
        assert_chart_exists(&html, "balances-chart");
        assert_chart_exists(&html, "expenses-chart");

        // Check that table is present
        assert_table_exists(&html);
    }

    #[tokio::test]
    async fn displays_prompt_text_on_no_data() {
        let conn = get_test_connection();
        let state = DashboardState {
            db_connection: Arc::new(Mutex::new(conn)),
            local_timezone: "Etc/UTC".to_owned(),
        };

        let response = get_dashboard_page(State(state)).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let html = parse_html(response).await;
        assert_tag_exclusion_controls_hidden(&html);
    }

    fn assert_tag_exclusion_controls_hidden(html: &Html) {
        assert_tag_exclusion_controls_visible(html, 0);
    }

    #[tokio::test]
    async fn displays_tag_exclusion_controls() {
        let conn = get_test_connection();
        // Need to add dummy transaction, otherwise the tag exclusions controls are hidden
        create_transaction(
            Transaction::build(1.00, OffsetDateTime::now_utc().date(), "test"),
            &conn,
        )
        .unwrap();
        create_tag(TagName::new("Food").unwrap(), &conn).unwrap();
        create_tag(TagName::new("Transport").unwrap(), &conn).unwrap();
        let state = DashboardState {
            db_connection: Arc::new(Mutex::new(conn)),
            local_timezone: "Etc/UTC".to_owned(),
        };

        let response = get_dashboard_page(State(state)).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let html = parse_html(response).await;
        assert_tag_exclusion_controls_visible(&html, 2);
    }

    fn assert_tag_exclusion_controls_visible(html: &Html, expected_count: usize) {
        let checkbox_selector =
            Selector::parse("input[type='checkbox'][name='excluded_tags']").unwrap();
        let checkboxes: Vec<_> = html.select(&checkbox_selector).collect();
        assert_eq!(
            checkboxes.len(),
            expected_count,
            "Should have {expected_count} tag checkboxes in {}",
            html.html()
        );
    }

    async fn parse_html(response: Response<Body>) -> Html {
        let body = response.into_body();
        let body = axum::body::to_bytes(body, usize::MAX).await.unwrap();
        let text = String::from_utf8_lossy(&body).to_string();

        Html::parse_document(&text)
    }

    #[track_caller]
    fn assert_valid_html(html: &Html) {
        assert!(
            html.errors.is_empty(),
            "Got HTML parsing errors: {:?}",
            html.errors
        );
    }

    #[track_caller]
    fn assert_chart_exists(html: &Html, chart_id: &str) {
        let selector = Selector::parse(&format!("#{}", chart_id)).unwrap();
        assert!(
            html.select(&selector).next().is_some(),
            "Chart with id '{}' not found",
            chart_id
        );
    }

    #[track_caller]
    fn assert_table_exists(html: &Html) {
        let selector = Selector::parse("table").unwrap();
        assert!(
            html.select(&selector).next().is_some(),
            "Monthly summary table not found"
        );
    }

    #[test]
    fn excluded_tags_form_handles_multiple_values() {
        // Test multiple values
        let form_data = "excluded_tags=2&excluded_tags=3&excluded_tags=5";
        let form: ExcludedTagsForm = serde_html_form::from_str(form_data).unwrap();
        assert_eq!(form.excluded_tags, vec![2, 3, 5]);

        // Test single value
        let form_data = "excluded_tags=2";
        let form: ExcludedTagsForm = serde_html_form::from_str(form_data).unwrap();
        assert_eq!(form.excluded_tags, vec![2]);

        // Test no values (when no checkboxes are selected)
        let form_data = "";
        let form: ExcludedTagsForm = serde_html_form::from_str(form_data).unwrap();
        assert_eq!(form.excluded_tags, Vec::<i64>::new());
    }
}
