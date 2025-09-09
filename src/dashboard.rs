//! This file defines the dashboard route and its handlers.

use askama::Template;
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use axum_extra::extract::Form;
use rusqlite::{Connection, params_from_iter};
use serde::Deserialize;
use std::ops::RangeInclusive;
use time::{Date, Duration, OffsetDateTime};

use crate::{
    Error,
    balances::get_total_account_balance,
    dashboard_preferences::{get_excluded_tags, save_excluded_tags},
    database_id::DatabaseID,
    endpoints,
    navigation::{NavbarTemplate, get_nav_bar},
    shared_templates::render,
    state::DashboardState,
    tag::{Tag, get_all_tags},
};

// ============================================================================
// CONSTANTS
// ============================================================================

/// Number of days to look back for monthly summary calculations
const MONTHLY_PERIOD_DAYS: i64 = 28;

/// Number of days to look back for yearly summary calculations  
const YEARLY_PERIOD_DAYS: i64 = 365;

// ============================================================================
// MODELS
// ============================================================================

/// A summary of transaction amounts over a period, with income, expenses, and net income.
#[derive(Debug, Clone, PartialEq)]
pub struct TransactionSummary {
    /// Total positive transaction amounts (income)
    pub income: f64,
    /// Total negative transaction amounts (expenses, as absolute value)
    pub expenses: f64,
    /// Net income (income - expenses)
    pub net_income: f64,
}

/// A tag with its exclusion status for dashboard display
#[derive(Debug, Clone)]
pub struct TagWithExclusion {
    /// The tag
    pub tag: Tag,
    /// Whether this tag is currently excluded from dashboard summaries
    pub is_excluded: bool,
}

/// Summary data used by dashboard templates
#[derive(Debug, Clone)]
struct DashboardSummaryData {
    /// Summary of transactions for the last 28 days.
    monthly_summary: TransactionSummary,
    /// Summary of transactions for the last 12 months.
    yearly_summary: TransactionSummary,
    /// Total balance across all accounts.
    total_account_balance: f64,
}

// ============================================================================
// DATABASE FUNCTIONS
// ============================================================================

/// Get a summary of transactions (income, expenses, net income) within a date range.
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
pub fn get_transaction_summary(
    date_range: RangeInclusive<Date>,
    excluded_tags: Option<&[DatabaseID]>,
    connection: &Connection,
) -> Result<TransactionSummary, Error> {
    let base_query = "SELECT 
        COALESCE(SUM(CASE WHEN amount > 0 THEN amount ELSE 0 END), 0) as income,
        COALESCE(SUM(CASE WHEN amount < 0 THEN -amount ELSE 0 END), 0) as expenses
    FROM \"transaction\" t
    WHERE t.date BETWEEN ?1 AND ?2";

    let (query, params) = if let Some(tags) = excluded_tags.filter(|t| !t.is_empty()) {
        let placeholders = tags.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let query_with_exclusions = format!(
            "{base_query} AND t.id NOT IN (
                SELECT tt.transaction_id 
                FROM transaction_tag tt 
                WHERE tt.tag_id IN ({placeholders})
            )"
        );

        let mut params = vec![date_range.start().to_string(), date_range.end().to_string()];
        params.extend(tags.iter().map(|tag| tag.to_string()));
        (query_with_exclusions, params)
    } else {
        (
            base_query.to_string(),
            vec![date_range.start().to_string(), date_range.end().to_string()],
        )
    };

    let mut stmt = connection.prepare(&query)?;
    let (income, expenses): (f64, f64) = stmt.query_row(params_from_iter(params), |row| {
        Ok((row.get(0)?, row.get(1)?))
    })?;

    Ok(TransactionSummary {
        income,
        expenses,
        net_income: income - expenses,
    })
}

// ============================================================================
// TEMPLATES AND HANDLERS
// ============================================================================

/// Renders the dashboard page.
#[derive(Template)]
#[template(path = "views/dashboard.html")]
struct DashboardTemplate<'a> {
    nav_bar: NavbarTemplate<'a>,
    /// Summary data (monthly, yearly, balance)
    summary_data: DashboardSummaryData,
    /// All available tags with their exclusion status
    tags_with_status: Vec<TagWithExclusion>,
    /// API endpoint for updating excluded tags
    excluded_tags_endpoint: &'a str,
}

/// Display a page with an overview of the user's data.
pub async fn get_dashboard_page(State(state): State<DashboardState>) -> Response {
    let nav_bar = get_nav_bar(endpoints::DASHBOARD_VIEW);

    let today = OffsetDateTime::now_utc().date();
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

    // Calculate monthly summary (last 28 days)
    let one_month_ago = today - Duration::days(MONTHLY_PERIOD_DAYS);
    let monthly_summary =
        match get_transaction_summary(one_month_ago..=today, excluded_tags_slice, &connection) {
            Ok(summary) => summary,
            Err(error) => return error.into_response(),
        };

    // Calculate yearly summary (last 365 days)
    let one_year_ago = today - Duration::days(YEARLY_PERIOD_DAYS);
    let yearly_summary =
        match get_transaction_summary(one_year_ago..=today, excluded_tags_slice, &connection) {
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
        DashboardTemplate {
            nav_bar,
            summary_data: DashboardSummaryData {
                monthly_summary,
                yearly_summary,
                total_account_balance,
            },
            tags_with_status,
            excluded_tags_endpoint: endpoints::DASHBOARD_EXCLUDED_TAGS,
        },
    )
}

// ============================================================================
// API ENDPOINTS
// ============================================================================

/// Form data for updating excluded tags
#[derive(Deserialize)]
pub struct ExcludedTagsForm {
    /// List of tag IDs to exclude from dashboard summaries
    #[serde(default)]
    pub excluded_tags: Vec<DatabaseID>,
}

/// Template for rendering just the dashboard summary sections
#[derive(Template)]
#[template(path = "partials/dashboard_summaries.html")]
struct DashboardSummariesTemplate {
    /// Summary data (monthly, yearly, balance)
    summary_data: DashboardSummaryData,
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

    // Get updated summaries
    let today = OffsetDateTime::now_utc().date();
    let excluded_tags_slice = if excluded_tags.is_empty() {
        None
    } else {
        Some(excluded_tags.as_slice())
    };

    // Calculate monthly summary (last 28 days)
    let one_month_ago = today - Duration::days(MONTHLY_PERIOD_DAYS);
    let monthly_summary =
        match get_transaction_summary(one_month_ago..=today, excluded_tags_slice, &connection) {
            Ok(summary) => summary,
            Err(_) => return Error::DashboardCalculationError.into_response(),
        };

    // Calculate yearly summary (last 365 days)
    let one_year_ago = today - Duration::days(YEARLY_PERIOD_DAYS);
    let yearly_summary =
        match get_transaction_summary(one_year_ago..=today, excluded_tags_slice, &connection) {
            Ok(summary) => summary,
            Err(_) => return Error::DashboardCalculationError.into_response(),
        };

    // Get total account balance
    let total_account_balance = match get_total_account_balance(&connection) {
        Ok(total) => total,
        Err(_) => return Error::DashboardCalculationError.into_response(),
    };

    render(
        StatusCode::OK,
        DashboardSummariesTemplate {
            summary_data: DashboardSummaryData {
                monthly_summary,
                yearly_summary,
                total_account_balance,
            },
        },
    )
}

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
        dashboard_preferences::save_excluded_tags,
        database_id::DatabaseID,
        db::initialize,
        state::DashboardState,
        tag::{TagName, create_tag},
        transaction::{Transaction, create_transaction},
        transaction_tag::set_transaction_tags,
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
    async fn dashboard_displays_monthly_and_yearly_summaries() {
        let conn = get_test_connection();
        let today = OffsetDateTime::now_utc().date();

        // Create transactions for monthly summary (within last 30 days)
        create_transaction(Transaction::build(100.0).date(today).unwrap(), &conn).unwrap();
        create_transaction(
            Transaction::build(-50.0)
                .date(today - Duration::days(15))
                .unwrap(),
            &conn,
        )
        .unwrap();

        // Create transactions for yearly summary (within last 365 days but outside monthly range)
        create_transaction(
            Transaction::build(200.0)
                .date(today - Duration::days(60))
                .unwrap(),
            &conn,
        )
        .unwrap();
        create_transaction(
            Transaction::build(-100.0)
                .date(today - Duration::days(180))
                .unwrap(),
            &conn,
        )
        .unwrap();

        // Create account balances
        conn.execute(
            "INSERT INTO balance (account, balance, date) VALUES (?1, ?2, ?3)",
            ("Account 1", 500.0, today.to_string()),
        )
        .unwrap();
        conn.execute(
            "INSERT INTO balance (account, balance, date) VALUES (?1, ?2, ?3)",
            ("Account 2", 250.0, today.to_string()),
        )
        .unwrap();

        let state = DashboardState {
            db_connection: Arc::new(Mutex::new(conn)),
        };

        let response = get_dashboard_page(State(state)).await;

        assert_eq!(response.status(), StatusCode::OK);

        let html = parse_html(response).await;
        assert_valid_html(&html);

        // Check monthly summary section
        let monthly_section = get_section_by_heading(&html, "Last 28 Days");
        assert_section_contains_values(&monthly_section, &["$100", "$50", "$50"]);

        // Check yearly summary section
        let yearly_section = get_section_by_heading(&html, "Last 12 Months");
        assert_section_contains_values(&yearly_section, &["$300", "$150", "$150"]);

        // Check total account balance section
        let balance_section = get_section_by_heading(&html, "Total Account Balance");
        assert_section_contains_value(&balance_section, "$750");
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
    fn get_section_by_heading<'a>(html: &'a Html, heading_text: &str) -> scraper::ElementRef<'a> {
        let heading_selector = Selector::parse("h3").unwrap();

        for heading in html.select(&heading_selector) {
            let text: String = heading.text().collect();
            if text.trim() == heading_text {
                // Find the parent div containing this heading
                if let Some(parent) = heading.parent() {
                    if let Some(section) = scraper::ElementRef::wrap(parent) {
                        if section.value().name() == "div" {
                            return section;
                        }
                    }
                }
            }
        }
        panic!("Could not find section with heading '{}'", heading_text);
    }

    #[track_caller]
    fn assert_section_contains_values(section: &scraper::ElementRef, expected_values: &[&str]) {
        let text: String = section.text().collect();
        for expected in expected_values {
            assert!(
                text.contains(expected),
                "Section should contain '{}' but got: {}",
                expected,
                text
            );
        }
    }

    #[track_caller]
    fn assert_section_contains_value(section: &scraper::ElementRef, expected_value: &str) {
        assert_section_contains_values(section, &[expected_value]);
    }

    #[tokio::test]
    async fn dashboard_excludes_tagged_transactions_from_summaries() {
        let conn = get_test_connection();
        let today = OffsetDateTime::now_utc().date();

        // Create test tags
        let excluded_tag = create_tag(TagName::new("ExcludedTag").unwrap(), &conn).unwrap();
        let included_tag = create_tag(TagName::new("IncludedTag").unwrap(), &conn).unwrap();

        // Create transactions
        let excluded_transaction =
            create_transaction(Transaction::build(100.0).date(today).unwrap(), &conn).unwrap();
        let included_transaction =
            create_transaction(Transaction::build(50.0).date(today).unwrap(), &conn).unwrap();
        let _untagged_transaction =
            create_transaction(Transaction::build(25.0).date(today).unwrap(), &conn).unwrap();

        // Tag transactions
        set_transaction_tags(excluded_transaction.id(), &[excluded_tag.id], &conn).unwrap();
        set_transaction_tags(included_transaction.id(), &[included_tag.id], &conn).unwrap();

        // Set excluded tags
        save_excluded_tags(vec![excluded_tag.id], &conn).unwrap();

        let state = DashboardState {
            db_connection: Arc::new(Mutex::new(conn)),
        };

        let response = get_dashboard_page(State(state)).await;
        assert_eq!(response.status(), StatusCode::OK);

        let html = parse_html(response).await;
        assert_valid_html(&html);

        // Check that monthly summary excludes the tagged transaction
        // Should only include $50 (included) + $25 (untagged) = $75, not $100 from excluded
        let monthly_section = get_section_by_heading(&html, "Last 28 Days");
        assert_section_contains_value(&monthly_section, "$75"); // Total income should be $75, not $175
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
        assert_eq!(form.excluded_tags, Vec::<DatabaseID>::new());
    }

    #[tokio::test]
    async fn dashboard_includes_all_transactions_when_no_tags_excluded() {
        let conn = get_test_connection();
        let today = OffsetDateTime::now_utc().date();

        // Create test tags
        let tag = create_tag(TagName::new("TestTag").unwrap(), &conn).unwrap();

        // Create transactions
        let tagged_transaction =
            create_transaction(Transaction::build(100.0).date(today).unwrap(), &conn).unwrap();
        let _untagged_transaction =
            create_transaction(Transaction::build(50.0).date(today).unwrap(), &conn).unwrap();

        // Tag one transaction
        set_transaction_tags(tagged_transaction.id(), &[tag.id], &conn).unwrap();

        // Don't exclude any tags (default state)

        let state = DashboardState {
            db_connection: Arc::new(Mutex::new(conn)),
        };

        let response = get_dashboard_page(State(state)).await;
        assert_eq!(response.status(), StatusCode::OK);

        let html = parse_html(response).await;
        assert_valid_html(&html);

        // Check that monthly summary includes all transactions
        let monthly_section = get_section_by_heading(&html, "Last 28 Days");
        assert_section_contains_value(&monthly_section, "$150"); // Total income should be $150
    }
}

#[cfg(test)]
mod get_transaction_summary_tests {
    use rusqlite::Connection;
    use time::macros::date;

    use super::{TransactionSummary, get_transaction_summary};
    use crate::{
        db::initialize,
        tag::{TagName, create_tag},
        transaction::{Transaction, create_transaction},
        transaction_tag::set_transaction_tags,
    };

    fn get_test_connection() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();
        conn
    }

    #[test]
    fn returns_summary_with_income_and_expenses() {
        let conn = get_test_connection();
        let start_date = date!(2024 - 01 - 01);
        let end_date = date!(2024 - 01 - 31);

        // Create test transactions
        create_transaction(Transaction::build(100.0).date(start_date).unwrap(), &conn).unwrap();
        create_transaction(
            Transaction::build(-50.0)
                .date(date!(2024 - 01 - 15))
                .unwrap(),
            &conn,
        )
        .unwrap();
        create_transaction(Transaction::build(75.0).date(end_date).unwrap(), &conn).unwrap();
        create_transaction(
            Transaction::build(-25.0)
                .date(date!(2024 - 01 - 20))
                .unwrap(),
            &conn,
        )
        .unwrap();

        let result = get_transaction_summary(start_date..=end_date, None, &conn).unwrap();

        assert_eq!(result.income, 175.0);
        assert_eq!(result.expenses, 75.0);
        assert_eq!(result.net_income, 100.0);
    }

    #[test]
    fn returns_zero_summary_for_no_transactions() {
        let conn = get_test_connection();
        let start_date = date!(2024 - 01 - 01);
        let end_date = date!(2024 - 01 - 31);

        let result = get_transaction_summary(start_date..=end_date, None, &conn).unwrap();

        let expected = TransactionSummary {
            income: 0.0,
            expenses: 0.0,
            net_income: 0.0,
        };

        assert_eq!(result, expected);
    }

    #[test]
    fn excludes_transactions_outside_date_range() {
        let conn = get_test_connection();
        let start_date = date!(2024 - 01 - 01);
        let end_date = date!(2024 - 01 - 31);

        // Transactions within range
        create_transaction(Transaction::build(100.0).date(start_date).unwrap(), &conn).unwrap();
        create_transaction(Transaction::build(-50.0).date(end_date).unwrap(), &conn).unwrap();

        // Transactions outside range
        create_transaction(
            Transaction::build(200.0)
                .date(date!(2023 - 12 - 31))
                .unwrap(),
            &conn,
        )
        .unwrap();
        create_transaction(
            Transaction::build(-100.0)
                .date(date!(2024 - 02 - 01))
                .unwrap(),
            &conn,
        )
        .unwrap();

        let result = get_transaction_summary(start_date..=end_date, None, &conn).unwrap();

        assert_eq!(result.income, 100.0);
        assert_eq!(result.expenses, 50.0);
        assert_eq!(result.net_income, 50.0);
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
        let excluded_transaction =
            create_transaction(Transaction::build(100.0).date(start_date).unwrap(), &conn).unwrap();
        let included_transaction =
            create_transaction(Transaction::build(50.0).date(start_date).unwrap(), &conn).unwrap();
        let _untagged_transaction =
            create_transaction(Transaction::build(25.0).date(start_date).unwrap(), &conn).unwrap();

        // Tag transactions
        set_transaction_tags(excluded_transaction.id(), &[excluded_tag.id], &conn).unwrap();
        set_transaction_tags(included_transaction.id(), &[included_tag.id], &conn).unwrap();

        // Get summary excluding the excluded tag
        let excluded_tags = vec![excluded_tag.id];
        let result =
            get_transaction_summary(start_date..=end_date, Some(&excluded_tags), &conn).unwrap();

        // Should only include $50 (tagged with included tag) + $25 (untagged) = $75
        // Should exclude $100 (tagged with excluded tag)
        assert_eq!(result.income, 75.0);
        assert_eq!(result.expenses, 0.0);
        assert_eq!(result.net_income, 75.0);
    }

    #[test]
    fn includes_all_transactions_when_no_tags_excluded() {
        let conn = get_test_connection();
        let start_date = date!(2024 - 01 - 01);
        let end_date = date!(2024 - 01 - 31);

        // Create test tag
        let tag = create_tag(TagName::new("TestTag").unwrap(), &conn).unwrap();

        // Create test transactions
        let tagged_transaction =
            create_transaction(Transaction::build(100.0).date(start_date).unwrap(), &conn).unwrap();
        let _untagged_transaction =
            create_transaction(Transaction::build(50.0).date(start_date).unwrap(), &conn).unwrap();

        // Tag one transaction
        set_transaction_tags(tagged_transaction.id(), &[tag.id], &conn).unwrap();

        // Get summary with no exclusions
        let result = get_transaction_summary(start_date..=end_date, None, &conn).unwrap();

        // Should include all transactions: $100 + $50 = $150
        assert_eq!(result.income, 150.0);
        assert_eq!(result.expenses, 0.0);
        assert_eq!(result.net_income, 150.0);
    }
}
