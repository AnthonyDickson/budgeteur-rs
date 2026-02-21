//! Dashboard HTTP handlers and view rendering.
//!
//! # HTMX Note
//! Charts must be initialized inline. See comment in `dashboard_view()`.

use axum::{
    extract::{FromRef, State},
    response::{IntoResponse, Response},
};
use axum_extra::extract::Form;
use maud::{Markup, PreEscaped, html};
use rusqlite::Connection;
use std::{
    ops::RangeInclusive,
    sync::{Arc, Mutex},
};
use time::{Date, Duration, Month, OffsetDateTime};

use crate::{
    AppState, Error,
    account::get_total_account_balance,
    dashboard::{
        aggregation::{TagExpenseStats, calculate_tag_expense_statistics},
        cards::expense_cards_view,
        charts::{
            DashboardChart, balances_chart, charts_inline_script, expenses_chart, net_income_chart,
        },
        tables::{monthly_summary_table, summary_statistics_table},
        transaction::{Transaction, get_transactions_in_date_range},
    },
    endpoints,
    html::{HeadElement, base, link},
    navigation::NavBar,
    tag::{
        ExcludedTagsForm, ExcludedTagsViewConfig, TagId, TagWithExclusion,
        build_tags_with_exclusion_status, excluded_tags_controls, get_all_tags, get_excluded_tags,
        save_excluded_tags,
    },
    timezone::get_local_offset,
};

/// Number of days to look back for yearly summary calculations  
const YEARLY_PERIOD_DAYS: i64 = 365; // Could be 366 for leap years, but using 365 for consistency

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

/// Holds all the data needed to render the dashboard.
struct DashboardData {
    tags_with_status: Vec<TagWithExclusion>,
    charts: [DashboardChart; 3],
    tables: Vec<Markup>,
    tag_stats: Vec<TagExpenseStats>,
    displayed_month: Date,
}

/// Display a page with an overview of the user's data.
pub async fn get_dashboard_page(State(state): State<DashboardState>) -> Result<Response, Error> {
    let connection = state
        .db_connection
        .lock()
        .inspect_err(|error| tracing::error!("could not acquire database lock: {error}"))
        .map_err(|_| Error::DatabaseLockError)?;

    let nav_bar = NavBar::new(endpoints::DASHBOARD_VIEW);

    let excluded_tag_ids = get_excluded_tags(&connection)
        .inspect_err(|error| tracing::error!("could not get excluded tags: {error}"))?;

    match build_dashboard_data(&excluded_tag_ids, &state.local_timezone, &connection)? {
        Some(data) => Ok(dashboard_view(
            nav_bar,
            &data.tags_with_status,
            &data.charts,
            &data.tables,
            &data.tag_stats,
            data.displayed_month,
        )
        .into_response()),
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
            // This shouldn't happen when updating filters, only on initial load
            tracing::warn!("No transaction data after updating excluded tags");
            return html! {
                div class="text-center text-gray-600 dark:text-gray-400" {
                    "No transaction data available"
                }
            }
            .into_response();
        }
        Err(error) => {
            tracing::error!("Failed to build dashboard data: {error}");
            return error.into_alert_response();
        }
    };

    dashboard_content_partial(
        &data.tags_with_status,
        &data.charts,
        &data.tables,
        &data.tag_stats,
        data.displayed_month,
    )
    .into_response()
}

/// Gets the date range for dashboard queries (last year from today).
///
/// # Arguments
/// * `today` - The reference date to calculate the range from
///
/// # Returns
/// Inclusive date range for the last twelve months including the current month.
fn last_twelve_months(today: Date) -> RangeInclusive<Date> {
    let start = today - Duration::days(YEARLY_PERIOD_DAYS);

    let start = match start.month() {
        Month::December => Date::from_calendar_date(start.year() + 1, Month::January, 1)
            .expect("could not create date"),
        month if month == today.month() => start
            .replace_day(1)
            .and_then(|d| d.replace_month(month.next()))
            .expect("could not create date"),
        // This case may happen on 29 Feb on a leap year.
        // For example, 29 Feb 2028 - 365 days = 01 Mar 2027
        _ => start,
    };

    start..=today
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
    excluded_tag_ids: &[TagId],
    local_timezone_name: &str,
    connection: &Connection,
) -> Result<Option<DashboardData>, Error> {
    let available_tags = get_all_tags(connection)
        .inspect_err(|error| tracing::error!("could not get tags: {error}"))?;

    let tags_with_status = build_tags_with_exclusion_status(available_tags, excluded_tag_ids);

    let local_timezone = get_local_offset(local_timezone_name).ok_or_else(|| {
        tracing::error!("Invalid timezone {}", local_timezone_name);
        Error::InvalidTimezoneError(local_timezone_name.to_owned())
    })?;

    let excluded_tags_slice = if excluded_tag_ids.is_empty() {
        None
    } else {
        Some(excluded_tag_ids)
    };

    let today = OffsetDateTime::now_utc().to_offset(local_timezone).date();
    let date_range = last_twelve_months(today);
    let transactions = get_transactions_in_date_range(date_range, excluded_tags_slice, connection)
        .inspect_err(|error| {
            tracing::error!("Could not get transactions for last year: {error}")
        })?;

    if transactions.is_empty() {
        return Ok(None);
    }

    let total_account_balance = get_total_account_balance(connection).inspect_err(|error| {
        tracing::error!("Could not calculate total account balance: {error}")
    })?;

    let charts = build_dashboard_charts(&transactions, total_account_balance);
    let tables = vec![
        summary_statistics_table(&transactions, total_account_balance),
        monthly_summary_table(&transactions, total_account_balance),
    ];

    // Safe unwrap: dates from OffsetDateTime are always valid
    let last_complete_month = (today.replace_day(1).unwrap() - Duration::days(1))
        .replace_day(1)
        .unwrap();
    let tag_stats = calculate_tag_expense_statistics(&transactions, last_complete_month);

    Ok(Some(DashboardData {
        tags_with_status,
        charts,
        tables,
        tag_stats,
        displayed_month: last_complete_month,
    }))
}

/// Creates the array of dashboard charts from transaction data.
///
/// Generates three charts: net income, balances, and expenses by tag.
/// The chart options are serialized to JSON for ECharts consumption.
///
/// # Arguments
/// * `transactions` - Transaction data for the last 12 months
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
/// * `tag_stats` - Expense cards to display
fn dashboard_view<'a>(
    nav_bar: NavBar<'a>,
    tags_with_status: &[TagWithExclusion],
    charts: &[DashboardChart],
    tables: &[Markup],
    tag_stats: &[TagExpenseStats],
    displayed_month: Date,
) -> Markup {
    let nav_bar = nav_bar.into_html();
    let content =
        dashboard_content_partial(tags_with_status, charts, tables, tag_stats, displayed_month);

    let scripts = [
        HeadElement::ScriptLink("/static/echarts.6.0.0.min.js".to_owned()),
        HeadElement::ScriptLink("/static/echarts-gl.2.0.9.min.js".to_owned()),
    ];

    base(
        "Dashboard",
        &scripts,
        &html!(
         (nav_bar)
            div
                id="dashboard-content"
                class="flex flex-col items-center px-2 lg:px-6 lg:py-8 mx-auto
                    max-w-screen-xl text-gray-900 dark:text-white"
            {
                (content)
            }
        ),
    )
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
    tag_stats: &[TagExpenseStats],
    displayed_month: Date,
) -> Markup {
    let excluded_tags_view = excluded_tags_controls(
        tags_with_status,
        ExcludedTagsViewConfig {
            heading: "Filter Out Tags",
            description: "Exclude transactions with these tags from the charts and table above:",
            endpoint: endpoints::DASHBOARD_EXCLUDED_TAGS,
            hx_target: Some("#dashboard-content"),
            hx_swap: Some("innerHTML"),
            hx_trigger: Some("change"),
            redirect_url: None,
            form_id: None,
        },
    );
    let expense_cards = expense_cards_view(tag_stats, displayed_month);

    html!(
        section
            id="charts"
            class="w-full mx-auto mb-4"
        {
            div class="grid grid-cols-1 lg:grid-cols-2 gap-4"
            {
                @for chart in charts {
                    div
                        id=(chart.id)
                        class="min-h-[240px] sm:min-h-[300px] md:min-h-[340px] lg:min-h-[380px] rounded dark:bg-gray-100"
                    {}
                }
            }
        }

        section
            id="tables"
            class="w-full mx-auto mb-4"
        {
            div class="grid grid-cols-1 lg:grid-cols-2 gap-4"
            {
                @for table in tables {
                    (table)
                }
            }
        }

        (expense_cards)

        // ⚠️ CRITICAL: Charts must be initialized inline, not in <head>
        // HTMX swaps don't trigger DOMContentLoaded. DO NOT MOVE.
        script {
            (PreEscaped(charts_inline_script(charts)))
        }

        (excluded_tags_view)
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
        dashboard::handlers::{DashboardState, last_twelve_months},
        db::initialize,
        tag::{TagId, TagName, create_tag},
        transaction::{Transaction, create_transaction},
    };

    use rusqlite::Connection;
    use std::sync::{Arc, Mutex};

    use super::get_dashboard_page;
    use crate::tag::ExcludedTagsForm;

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
        assert_eq!(form.excluded_tags, Vec::<TagId>::new());
    }

    #[test]
    fn last_twelve_months_returns_correct_range() {
        use time::{Date, Month};

        let test_cases = [
            // (today, expected_start)
            (
                Date::from_calendar_date(2026, Month::January, 15).unwrap(),
                Date::from_calendar_date(2025, Month::February, 1).unwrap(),
            ),
            (
                Date::from_calendar_date(2026, Month::March, 5).unwrap(),
                Date::from_calendar_date(2025, Month::April, 1).unwrap(),
            ),
            (
                Date::from_calendar_date(2026, Month::December, 15).unwrap(),
                Date::from_calendar_date(2026, Month::January, 1).unwrap(),
            ),
            (
                Date::from_calendar_date(2028, Month::February, 29).unwrap(),
                Date::from_calendar_date(2027, Month::March, 1).unwrap(),
            ),
        ];

        for (today, expected_start) in test_cases {
            let range = last_twelve_months(today);

            assert_eq!(
                *range.start(),
                expected_start,
                "For today={}, expected start={}, got start={}",
                today,
                expected_start,
                range.start()
            );

            assert_eq!(
                *range.end(),
                today,
                "For today={}, expected end={}, got end={}",
                today,
                today,
                range.end()
            );
        }
    }
}
