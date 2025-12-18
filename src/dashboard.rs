//! This file defines the dashboard route and its handlers.

use askama::Template;
use axum::{
    extract::{FromRef, State},
    http::{StatusCode, Uri},
    response::{IntoResponse, Response},
};
use axum_extra::extract::Form;
use charming::{
    Chart,
    component::{Axis, Grid, Legend, Title, VisualMap, VisualMapPiece},
    element::{
        AxisLabel, AxisPointer, AxisPointerType, AxisType, Emphasis, EmphasisFocus, JsFunction,
        Tooltip, Trigger,
    },
    series::{Line, bar},
};
use rusqlite::{Connection, params_from_iter};
use serde::Deserialize;
use std::{
    collections::HashMap,
    ops::RangeInclusive,
    sync::{Arc, Mutex},
};
use time::{Date, Duration, OffsetDateTime};

use crate::{
    AppState, Error,
    account::get_total_account_balance,
    dashboard_preferences::{get_excluded_tags, save_excluded_tags},
    database_id::DatabaseId,
    endpoints,
    navigation::{NavbarTemplate, get_nav_bar},
    shared_templates::render,
    tag::{Tag, get_all_tags},
    timezone::get_local_offset,
};

// ============================================================================
// CONSTANTS
// ============================================================================

/// Number of days to look back for yearly summary calculations  
const YEARLY_PERIOD_DAYS: i64 = 365;

// ============================================================================
// MODELS
// ============================================================================

/// A tag with its exclusion status for dashboard display
#[derive(Debug, Clone)]
pub struct TagWithExclusion {
    /// The tag
    pub tag: Tag,
    /// Whether this tag is currently excluded from dashboard summaries
    pub is_excluded: bool,
}

#[derive(Debug)]
struct Transaction {
    amount: f64,
    date: Date,
    tag: String,
}

// ============================================================================
// DATABASE FUNCTIONS
// ============================================================================

/// Get transactions and their tags within a date range.
///
/// # Arguments
/// * `date_range` - The inclusive date range to summarize transactions for
/// * `excluded_tags` - Optional slice of tag IDs to exclude from the summary
/// * `connection` - Database connection reference
///
/// # Errors
/// Returns [Error::SqlError] if:
/// - Database connection fails
/// - SQL query preparation or execution fails
fn get_transactions_in_date_range(
    date_range: RangeInclusive<Date>,
    excluded_tags: Option<&[DatabaseId]>,
    connection: &Connection,
) -> Result<Vec<Transaction>, Error> {
    let base_query = "SELECT 
        t.amount,
        t.date,
        COALESCE(tag.name, 'Other') AS tag_name
    FROM \"transaction\" t
    LEFT JOIN tag ON tag.id = t.tag_id
    WHERE t.date BETWEEN ?1 AND ?2";

    let (query, params) = if let Some(tags) = excluded_tags.filter(|t| !t.is_empty()) {
        let placeholders = tags.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let query_without_excluded_tags =
            format!("{base_query} AND (t.tag_id IS NULL OR t.tag_id NOT IN ({placeholders}))");

        let mut params = vec![date_range.start().to_string(), date_range.end().to_string()];
        params.extend(tags.iter().map(|tag| tag.to_string()));
        (query_without_excluded_tags, params)
    } else {
        (
            base_query.to_owned(),
            vec![date_range.start().to_string(), date_range.end().to_string()],
        )
    };

    let mut stmt = connection.prepare(&query)?;
    stmt.query_map(params_from_iter(params), |row| {
        Ok(Transaction {
            amount: row.get(0)?,
            date: row.get(1)?,
            tag: row.get(2)?,
        })
    })?
    .collect::<Result<Vec<Transaction>, rusqlite::Error>>()
    .map_err(|error| error.into())
}

// ============================================================================
// TEMPLATES AND HANDLERS
// ============================================================================

/// The state needed for displaying the dashboard page.
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

struct DashboardChart<'a> {
    /// The HTML element ID to use for the chart (kebab case)
    id: &'a str,
    /// The JSON object for echarts
    options: &'a str,
}

/// Renders the dashboard charts section.
#[derive(Template)]
#[template(path = "partials/dashboard_charts.html")]
struct DashboardChartsTemplate<'a> {
    charts: &'a [DashboardChart<'a>],
}

/// Renders the dashboard page.
#[derive(Template)]
#[template(path = "views/dashboard.html")]
struct DashboardTemplate<'a> {
    nav_bar: NavbarTemplate<'a>,

    /// All available tags with their exclusion status
    tags_with_status: Vec<TagWithExclusion>,
    /// API endpoint for updating excluded tags
    excluded_tags_endpoint: &'a str,

    charts: DashboardChartsTemplate<'a>,
}

/// Renders the dashboard page when there is no data to display.
#[derive(Template)]
#[template(path = "views/dashboard_empty.html")]
struct DashboardNoDataTemplate<'a> {
    nav_bar: NavbarTemplate<'a>,
    create_transaction_url: Uri,
    import_transaction_url: Uri,
}

/// Display a page with an overview of the user's data.
pub async fn get_dashboard_page(State(state): State<DashboardState>) -> Response {
    let nav_bar = get_nav_bar(endpoints::DASHBOARD_VIEW);

    let local_timezone = match get_local_offset(&state.local_timezone) {
        Some(offset) => offset,
        None => return Error::InvalidTimezoneError(state.local_timezone).into_response(),
    };
    let today = OffsetDateTime::now_utc().to_offset(local_timezone).date();
    let connection = state.db_connection.lock().unwrap();

    // Get available tags and excluded tags for dashboard summaries
    let available_tags = match get_all_tags(&connection) {
        Ok(tags) => tags,
        Err(error) => return error.into_response(),
    };

    let excluded_tag_ids = match get_excluded_tags(&connection) {
        Ok(tags) => tags,
        Err(error) => return error.into_response(),
    };

    // Create tags with exclusion status
    let tags_with_status: Vec<TagWithExclusion> = available_tags
        .into_iter()
        .map(|tag| TagWithExclusion {
            is_excluded: excluded_tag_ids.contains(&tag.id),
            tag,
        })
        .collect();

    let excluded_tags_slice = if excluded_tag_ids.is_empty() {
        None
    } else {
        Some(excluded_tag_ids.as_slice())
    };

    // Calculate yearly summary (last 365 days)
    let one_year_ago = today - Duration::days(YEARLY_PERIOD_DAYS);
    let transactions = match get_transactions_in_date_range(
        one_year_ago..=today,
        excluded_tags_slice,
        &connection,
    ) {
        Ok(summary) => summary,
        Err(error) => return error.into_response(),
    };

    if transactions.is_empty() {
        return render(
            StatusCode::OK,
            DashboardNoDataTemplate {
                nav_bar,
                create_transaction_url: Uri::from_static(endpoints::NEW_TRANSACTION_VIEW),
                import_transaction_url: Uri::from_static(endpoints::IMPORT_VIEW),
            },
        );
    }

    // Get total account balance
    let total_account_balance = match get_total_account_balance(&connection) {
        Ok(total) => total,
        Err(error) => return error.into_response(),
    };

    render(
        StatusCode::OK,
        DashboardTemplate {
            nav_bar,
            tags_with_status,
            excluded_tags_endpoint: endpoints::DASHBOARD_EXCLUDED_TAGS,
            charts: DashboardChartsTemplate {
                charts: &[
                    DashboardChart {
                        id: "net-income-chart",
                        options: &create_net_income_chart(&transactions).to_string(),
                    },
                    DashboardChart {
                        id: "balances-chart",
                        options: &create_balances_chart(total_account_balance, &transactions)
                            .to_string(),
                    },
                    DashboardChart {
                        id: "expenses-chart",
                        options: &create_expenses_chart(&transactions).to_string(),
                    },
                ],
            },
        },
    )
}

// ============================================================================
// CHARTS
// ============================================================================

fn create_net_income_chart(transactions: &[Transaction]) -> Chart {
    let monthly_totals = aggregate_by_month(transactions);
    let (labels, values) = prepare_chart_data(&monthly_totals);

    Chart::new()
        .title(
            Title::new()
                .text("Net income")
                .subtext("Last twelve months"),
        )
        .tooltip(create_currency_tooltip())
        .grid(
            Grid::new()
                .left("3%")
                .right("4%")
                .bottom("3%")
                .contain_label(true),
        )
        .x_axis(Axis::new().type_(AxisType::Category).data(labels))
        .y_axis(
            Axis::new()
                .type_(AxisType::Value)
                .axis_label(AxisLabel::new().formatter(get_chart_currency_formatter())),
        )
        .visual_map(VisualMap::new().show(false).pieces(vec![
            VisualMapPiece::new().lte(-1).color("red"),
            VisualMapPiece::new().gte(0).color("green"),
        ]))
        .series(Line::new().name("Net Income").data(values))
}

fn create_balances_chart(total_account_balance: f64, transactions: &[Transaction]) -> Chart {
    let monthly_totals = aggregate_by_month(transactions);
    let (labels, values) = calculate_running_balances(total_account_balance, &monthly_totals);

    Chart::new()
        .title(
            Title::new()
                .text("Net Balance")
                .subtext("Last twelve months"),
        )
        .tooltip(create_currency_tooltip())
        .grid(
            Grid::new()
                .left("3%")
                .right("4%")
                .bottom("3%")
                .contain_label(true),
        )
        .x_axis(Axis::new().type_(AxisType::Category).data(labels))
        .y_axis(
            Axis::new()
                .type_(AxisType::Value)
                .axis_label(AxisLabel::new().formatter(get_chart_currency_formatter())),
        )
        .series(Line::new().name("Balance").data(values))
}

fn create_expenses_chart(transactions: &[Transaction]) -> Chart {
    // Get all unique months from transactions and sort them
    let sorted_months = get_sorted_months(transactions);
    let labels = format_month_labels(&sorted_months);
    let series_data = group_monthly_expenses_by_tag(transactions, &sorted_months);

    let mut chart = Chart::new()
        .title(
            Title::new()
                .text("Monthly Expenses")
                .subtext("Last twelve months, grouped by tag"),
        )
        .tooltip(create_currency_tooltip())
        .legend(Legend::new().left(230).top(0))
        .grid(
            Grid::new()
                .left("3%")
                .right("4%")
                .bottom("3%")
                .top(90)
                .contain_label(true),
        )
        .x_axis(Axis::new().type_(AxisType::Category).data(labels))
        .y_axis(
            Axis::new()
                .type_(AxisType::Value)
                .axis_label(AxisLabel::new().formatter(get_chart_currency_formatter())),
        );

    for (tag, data) in series_data {
        chart = chart.series(
            bar::Bar::new()
                .name(tag)
                .stack("Expenses")
                .emphasis(Emphasis::new().focus(EmphasisFocus::Series))
                .data(data),
        );
    }

    chart
}

// ============================================================================
// CHART HELPER FUNCTIONS
// ============================================================================

/// Aggregates transaction amounts by month
fn aggregate_by_month(transactions: &[Transaction]) -> HashMap<Date, f64> {
    let mut totals: HashMap<Date, f64> = HashMap::new();

    for transaction in transactions {
        let month = transaction.date.replace_day(1).unwrap();
        *totals.entry(month).or_insert(0.0) += transaction.amount;
    }

    totals
}

/// Gets unique months from transactions and returns them sorted
fn get_sorted_months(transactions: &[Transaction]) -> Vec<Date> {
    let mut months = std::collections::HashSet::new();

    for transaction in transactions {
        let month = transaction.date.replace_day(1).unwrap();
        months.insert(month);
    }

    let mut sorted: Vec<Date> = months.into_iter().collect();
    sorted.sort();
    sorted
}

/// Converts monthly data into sorted labels and values for charting
fn prepare_chart_data(monthly_totals: &HashMap<Date, f64>) -> (Vec<String>, Vec<f64>) {
    let mut sorted_months: Vec<Date> = monthly_totals.keys().copied().collect();
    sorted_months.sort();

    let labels = format_month_labels(&sorted_months);
    let values = sorted_months
        .iter()
        .map(|month| monthly_totals[month])
        .collect();

    (labels, values)
}

/// Formats month labels as three-letter abbreviations
fn format_month_labels(months: &[Date]) -> Vec<String> {
    months
        .iter()
        .map(|date| {
            let mut month = date.month().to_string();
            month.truncate(3);
            month
        })
        .collect()
}

/// Calculates running balances by working backwards from the current total
fn calculate_running_balances(
    total_balance: f64,
    monthly_totals: &HashMap<Date, f64>,
) -> (Vec<String>, Vec<f64>) {
    let mut sorted_months: Vec<Date> = monthly_totals.keys().copied().collect();
    sorted_months.sort();

    let labels = format_month_labels(&sorted_months);

    // Calculate balances by working backwards from current total
    let mut balances = Vec::with_capacity(sorted_months.len());
    let mut cumulative = 0.0;

    for month in sorted_months.iter().rev() {
        balances.push(total_balance - cumulative);
        cumulative += monthly_totals[month];
    }

    balances.reverse();

    (labels, balances)
}

/// Groups expense transactions by tag and aggregates them by month
/// Groups expense transactions by tag and aggregates them by month
fn group_monthly_expenses_by_tag(
    transactions: &[Transaction],
    sorted_months: &[Date],
) -> Vec<(String, Vec<Option<f64>>)> {
    // Group transactions by tag
    let mut transactions_by_tag: HashMap<&str, Vec<&Transaction>> = HashMap::new();

    for transaction in transactions.iter().filter(|t| t.amount < 0.0) {
        transactions_by_tag
            .entry(transaction.tag.as_str())
            .or_default()
            .push(transaction);
    }

    // Sort tags, with "Other" at the end
    let mut sorted_tags: Vec<&str> = transactions_by_tag
        .keys()
        .copied()
        .filter(|&tag| tag != "Other")
        .collect();
    sorted_tags.sort();

    if transactions_by_tag.contains_key("Other") {
        sorted_tags.push("Other");
    }

    // Calculate monthly totals for each tag
    sorted_tags
        .into_iter()
        .map(|tag| {
            let monthly_data =
                calculate_monthly_expenses(transactions_by_tag[tag].as_slice(), sorted_months);
            (tag.to_owned(), monthly_data)
        })
        .collect()
}

/// Calculates monthly expense totals for a set of transactions
fn calculate_monthly_expenses(
    transactions: &[&Transaction],
    sorted_months: &[Date],
) -> Vec<Option<f64>> {
    let mut totals_by_month: HashMap<Date, f64> = HashMap::new();

    for transaction in transactions {
        let month = transaction.date.replace_day(1).unwrap();
        let amount = transaction.amount.abs();
        *totals_by_month.entry(month).or_insert(0.0) += amount;
    }

    sorted_months
        .iter()
        .map(|month| totals_by_month.get(month).copied())
        .collect()
}

#[inline]
fn get_chart_currency_formatter() -> JsFunction {
    JsFunction::new_with_args(
        "number",
        // Use USD instead of NZD since it is easier to read (No 'NZ' prefix)
        "const currencyFormatter = new Intl.NumberFormat('en-US', {
              style: 'currency',
              currency: 'USD'
            });
            return (number) ? currencyFormatter.format(number) : \"-\";",
    )
}

/// Creates a tooltip configuration for currency values
fn create_currency_tooltip() -> Tooltip {
    Tooltip::new()
        .trigger(Trigger::Axis)
        .value_formatter(get_chart_currency_formatter())
        .axis_pointer(AxisPointer::new().type_(AxisPointerType::Shadow))
}

// ============================================================================
// API ENDPOINTS
// ============================================================================

/// Form data for updating excluded tags
#[derive(Deserialize)]
pub struct ExcludedTagsForm {
    /// List of tag IDs to exclude from dashboard summaries
    #[serde(default)]
    pub excluded_tags: Vec<DatabaseId>,
}

/// API endpoint to update excluded tags and return updated summaries
pub async fn update_excluded_tags(
    State(state): State<DashboardState>,
    Form(form): Form<ExcludedTagsForm>,
) -> Response {
    let connection = state.db_connection.lock().unwrap();

    // Save the excluded tags
    let excluded_tags = form.excluded_tags;
    if save_excluded_tags(excluded_tags.clone(), &connection).is_err() {
        return Error::DashboardPreferencesSaveError.into_response();
    }

    // Get updated charts
    let local_timezone = match get_local_offset(&state.local_timezone) {
        Some(offset) => offset,
        None => return Error::InvalidTimezoneError(state.local_timezone).into_response(),
    };
    let today = OffsetDateTime::now_utc().to_offset(local_timezone).date();
    let excluded_tags_slice = if excluded_tags.is_empty() {
        None
    } else {
        Some(excluded_tags.as_slice())
    };

    let one_year_ago = today - Duration::days(YEARLY_PERIOD_DAYS);
    let transactions = match get_transactions_in_date_range(
        one_year_ago..=today,
        excluded_tags_slice,
        &connection,
    ) {
        Ok(summary) => summary,
        Err(error) => return error.into_response(),
    };

    // Get total account balance
    let total_account_balance = match get_total_account_balance(&connection) {
        Ok(total) => total,
        Err(error) => return error.into_response(),
    };

    render(
        StatusCode::OK,
        DashboardChartsTemplate {
            charts: &[
                DashboardChart {
                    id: "net-income-chart",
                    options: &create_net_income_chart(&transactions).to_string(),
                },
                DashboardChart {
                    id: "balances-chart",
                    options: &create_balances_chart(total_account_balance, &transactions)
                        .to_string(),
                },
                DashboardChart {
                    id: "expenses-chart",
                    options: &create_expenses_chart(&transactions).to_string(),
                },
            ],
        },
    )
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod dashboard_route_tests {
    use axum::{
        body::Body,
        extract::State,
        http::{Response, StatusCode},
    };
    use scraper::{Html, Selector};
    use time::{Duration, OffsetDateTime};

    use crate::{
        dashboard::DashboardState,
        database_id::DatabaseId,
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

        let response = get_dashboard_page(State(state)).await;

        assert_eq!(response.status(), StatusCode::OK);

        let html = parse_html(response).await;
        assert_valid_html(&html);

        // Check that charts are present
        assert_chart_exists(&html, "net-income-chart");
        assert_chart_exists(&html, "balances-chart");
        assert_chart_exists(&html, "expenses-chart");
    }

    #[tokio::test]
    async fn displays_prompt_text_on_no_data() {
        let conn = get_test_connection();
        let state = DashboardState {
            db_connection: Arc::new(Mutex::new(conn)),
            local_timezone: "Etc/UTC".to_owned(),
        };

        let response = get_dashboard_page(State(state)).await;
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

        let response = get_dashboard_page(State(state)).await;
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
        assert_eq!(form.excluded_tags, Vec::<DatabaseId>::new());
    }
}

#[cfg(test)]
mod get_transactions_in_date_range_tests {
    use rusqlite::Connection;
    use time::macros::date;

    use super::get_transactions_in_date_range;
    use crate::{
        db::initialize,
        tag::{TagName, create_tag},
        transaction::{Transaction, create_transaction},
    };

    fn get_test_connection() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();
        conn
    }

    #[test]
    fn returns_transactions_in_date_range() {
        let conn = get_test_connection();
        let start_date = date!(2024 - 01 - 01);
        let end_date = date!(2024 - 01 - 31);

        // Create test transactions
        create_transaction(Transaction::build(100.0, start_date, ""), &conn).unwrap();
        create_transaction(Transaction::build(-50.0, date!(2024 - 01 - 15), ""), &conn).unwrap();
        create_transaction(Transaction::build(75.0, end_date, ""), &conn).unwrap();

        let transactions =
            get_transactions_in_date_range(start_date..=end_date, None, &conn).unwrap();

        assert_eq!(transactions.len(), 3);

        // Verify amounts are correct
        let total: f64 = transactions.iter().map(|t| t.amount).sum();
        assert_eq!(total, 125.0); // 100 - 50 + 75
    }

    #[test]
    fn returns_empty_vec_for_no_transactions() {
        let conn = get_test_connection();
        let start_date = date!(2024 - 01 - 01);
        let end_date = date!(2024 - 01 - 31);

        let transactions =
            get_transactions_in_date_range(start_date..=end_date, None, &conn).unwrap();

        assert_eq!(transactions.len(), 0);
    }

    #[test]
    fn excludes_transactions_outside_date_range() {
        let conn = get_test_connection();
        let start_date = date!(2024 - 01 - 01);
        let end_date = date!(2024 - 01 - 31);

        // Transactions within range
        create_transaction(Transaction::build(100.0, start_date, ""), &conn).unwrap();
        create_transaction(Transaction::build(-50.0, end_date, ""), &conn).unwrap();

        // Transactions outside range
        create_transaction(Transaction::build(200.0, date!(2023 - 12 - 31), ""), &conn).unwrap();
        create_transaction(Transaction::build(-100.0, date!(2024 - 02 - 01), ""), &conn).unwrap();

        let transactions =
            get_transactions_in_date_range(start_date..=end_date, None, &conn).unwrap();

        assert_eq!(transactions.len(), 2);
        let total: f64 = transactions.iter().map(|t| t.amount).sum();
        assert_eq!(total, 50.0); // 100 - 50
    }

    #[test]
    fn excludes_transactions_with_excluded_tags() {
        let conn = get_test_connection();
        let start_date = date!(2024 - 01 - 01);
        let end_date = date!(2024 - 01 - 31);

        // Create test tags
        let excluded_tag = create_tag(TagName::new("ExcludedTag").unwrap(), &conn).unwrap();
        let included_tag = create_tag(TagName::new("IncludedTag").unwrap(), &conn).unwrap();

        // Create test transactions
        let _excluded_transaction = create_transaction(
            Transaction::build(100.0, start_date, "").tag_id(Some(excluded_tag.id)),
            &conn,
        )
        .unwrap();
        let _included_transaction = create_transaction(
            Transaction::build(50.0, start_date, "").tag_id(Some(included_tag.id)),
            &conn,
        )
        .unwrap();
        let _untagged_transaction =
            create_transaction(Transaction::build(25.0, start_date, ""), &conn).unwrap();

        // Get transactions excluding the excluded tag
        let excluded_tags = vec![excluded_tag.id];
        let transactions =
            get_transactions_in_date_range(start_date..=end_date, Some(&excluded_tags), &conn)
                .unwrap();

        assert_eq!(transactions.len(), 2, "Got transactions: {transactions:#?}");
        let total: f64 = transactions.iter().map(|t| t.amount).sum();
        assert_eq!(total, 75.0); // 50 + 25, excluding 100
    }

    #[test]
    fn includes_all_transactions_when_no_tags_excluded() {
        let conn = get_test_connection();
        let start_date = date!(2024 - 01 - 01);
        let end_date = date!(2024 - 01 - 31);

        // Create test tag
        let tag = create_tag(TagName::new("TestTag").unwrap(), &conn).unwrap();

        // Create test transactions
        let _tagged_transaction = create_transaction(
            Transaction::build(100.0, start_date, "").tag_id(Some(tag.id)),
            &conn,
        )
        .unwrap();
        let _untagged_transaction =
            create_transaction(Transaction::build(50.0, start_date, ""), &conn).unwrap();

        // Get transactions with no exclusions
        let transactions =
            get_transactions_in_date_range(start_date..=end_date, None, &conn).unwrap();

        assert_eq!(transactions.len(), 2);
        let total: f64 = transactions.iter().map(|t| t.amount).sum();
        assert_eq!(total, 150.0); // 100 + 50
    }

    #[test]
    fn assigns_other_tag_to_untagged_transactions() {
        let conn = get_test_connection();
        let start_date = date!(2024 - 01 - 01);
        let end_date = date!(2024 - 01 - 31);

        // Create untagged transaction
        create_transaction(Transaction::build(100.0, start_date, ""), &conn).unwrap();

        let transactions =
            get_transactions_in_date_range(start_date..=end_date, None, &conn).unwrap();

        assert_eq!(transactions.len(), 1);
        assert_eq!(transactions[0].tag, "Other");
    }
}

#[cfg(test)]
mod chart_function_tests {
    use time::macros::date;

    use super::{
        Transaction, aggregate_by_month, calculate_monthly_expenses, format_month_labels,
        get_sorted_months, group_monthly_expenses_by_tag,
    };

    fn create_test_transaction(amount: f64, date: time::Date, tag: &str) -> Transaction {
        Transaction {
            amount,
            date,
            tag: tag.to_owned(),
        }
    }

    #[test]
    fn aggregate_by_month_sums_transactions() {
        let transactions = vec![
            create_test_transaction(100.0, date!(2024 - 01 - 15), "Food"),
            create_test_transaction(50.0, date!(2024 - 01 - 20), "Transport"),
            create_test_transaction(-30.0, date!(2024 - 02 - 10), "Food"),
        ];

        let result = aggregate_by_month(&transactions);

        assert_eq!(result.len(), 2);
        assert_eq!(result[&date!(2024 - 01 - 01)], 150.0);
        assert_eq!(result[&date!(2024 - 02 - 01)], -30.0);
    }

    #[test]
    fn aggregate_by_month_handles_empty_input() {
        let transactions = vec![];
        let result = aggregate_by_month(&transactions);
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn get_sorted_months_returns_unique_sorted_months() {
        let transactions = vec![
            create_test_transaction(100.0, date!(2024 - 03 - 15), "Food"),
            create_test_transaction(50.0, date!(2024 - 01 - 20), "Transport"),
            create_test_transaction(-30.0, date!(2024 - 02 - 10), "Food"),
            create_test_transaction(25.0, date!(2024 - 01 - 25), "Other"), // Same month as second
        ];

        let result = get_sorted_months(&transactions);

        assert_eq!(result.len(), 3);
        assert_eq!(result[0], date!(2024 - 01 - 01));
        assert_eq!(result[1], date!(2024 - 02 - 01));
        assert_eq!(result[2], date!(2024 - 03 - 01));
    }

    #[test]
    fn format_month_labels_creates_three_letter_abbreviations() {
        let months = vec![
            date!(2024 - 01 - 01),
            date!(2024 - 02 - 01),
            date!(2024 - 12 - 01),
        ];

        let result = format_month_labels(&months);

        assert_eq!(result, vec!["Jan", "Feb", "Dec"]);
    }

    #[test]
    fn calculate_monthly_expenses_aggregates_by_month() {
        let t1 = create_test_transaction(-100.0, date!(2024 - 01 - 15), "Food");
        let t2 = create_test_transaction(-50.0, date!(2024 - 01 - 20), "Food");
        let t3 = create_test_transaction(-30.0, date!(2024 - 02 - 10), "Food");

        let transactions = vec![&t1, &t2, &t3];
        let months = vec![
            date!(2024 - 01 - 01),
            date!(2024 - 02 - 01),
            date!(2024 - 03 - 01),
        ];

        let result = calculate_monthly_expenses(&transactions, &months);

        assert_eq!(result.len(), 3);
        assert_eq!(result[0], Some(150.0)); // Jan: 100 + 50
        assert_eq!(result[1], Some(30.0)); // Feb: 30
        assert_eq!(result[2], None); // Mar: no data
    }

    #[test]
    fn group_monthly_expenses_by_tag_groups_correctly() {
        let transactions = vec![
            create_test_transaction(-100.0, date!(2024 - 01 - 15), "Food"),
            create_test_transaction(-50.0, date!(2024 - 01 - 20), "Transport"),
            create_test_transaction(-30.0, date!(2024 - 02 - 10), "Food"),
            create_test_transaction(200.0, date!(2024 - 01 - 10), "Income"), // Positive, should be ignored
        ];

        let months = vec![date!(2024 - 01 - 01), date!(2024 - 02 - 01)];

        let result = group_monthly_expenses_by_tag(&transactions, &months);

        // Should have 2 tags: Food and Transport (Income is positive, so excluded)
        assert_eq!(result.len(), 2);

        // Find Food tag
        let food_data = result.iter().find(|(tag, _)| tag == "Food").unwrap();
        assert_eq!(food_data.1, vec![Some(100.0), Some(30.0)]);

        // Find Transport tag
        let transport_data = result.iter().find(|(tag, _)| tag == "Transport").unwrap();
        assert_eq!(transport_data.1, vec![Some(50.0), None]);
    }

    #[test]
    fn group_monthly_expenses_by_tag_puts_other_last() {
        let transactions = vec![
            create_test_transaction(-100.0, date!(2024 - 01 - 15), "Zebra"),
            create_test_transaction(-50.0, date!(2024 - 01 - 20), "Other"),
            create_test_transaction(-30.0, date!(2024 - 01 - 10), "Alpha"),
        ];

        let months = vec![date!(2024 - 01 - 01)];

        let result = group_monthly_expenses_by_tag(&transactions, &months);

        assert_eq!(result.len(), 3);
        // Check that "Other" is last
        assert_eq!(result[2].0, "Other");
        // Check alphabetical order for others
        assert_eq!(result[0].0, "Alpha");
        assert_eq!(result[1].0, "Zebra");
    }

    #[test]
    fn group_monthly_expenses_by_tag_handles_no_other_tag() {
        let transactions = vec![
            create_test_transaction(-100.0, date!(2024 - 01 - 15), "Food"),
            create_test_transaction(-50.0, date!(2024 - 01 - 20), "Transport"),
        ];

        let months = vec![date!(2024 - 01 - 01)];

        let result = group_monthly_expenses_by_tag(&transactions, &months);

        // Should have 2 tags, neither is "Other"
        assert_eq!(result.len(), 2);
        assert!(result.iter().all(|(tag, _)| tag != "Other"));
    }
}
