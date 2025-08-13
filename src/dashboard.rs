//! This file defines the dashboard route and its handlers.

use askama_axum::Template;
use axum::{
    extract::State,
    response::{IntoResponse, Response},
};
use time::{Duration, OffsetDateTime};

use crate::{
    balances::get_total_account_balance,
    endpoints,
    navigation::{NavbarTemplate, get_nav_bar},
    state::DashboardState,
    transaction::{TransactionSummary, get_transaction_summary},
};

/// Renders the dashboard page.
#[derive(Template)]
#[template(path = "views/dashboard.html")]
struct DashboardTemplate<'a> {
    nav_bar: NavbarTemplate<'a>,
    /// Summary of transactions for the last 30 days.
    monthly_summary: TransactionSummary,
    /// Summary of transactions for the last 12 months.
    yearly_summary: TransactionSummary,
    /// Total balance across all accounts.
    total_account_balance: f64,
}

/// Display a page with an overview of the user's data.
pub async fn get_dashboard_page(State(state): State<DashboardState>) -> Response {
    let nav_bar = get_nav_bar(endpoints::DASHBOARD_VIEW);

    let today = OffsetDateTime::now_utc().date();
    let connection = state.db_connection.lock().unwrap();

    // Calculate monthly summary (last 30 days)
    let one_month_ago = today - Duration::days(30);
    let monthly_summary = match get_transaction_summary(one_month_ago..=today, &connection) {
        Ok(summary) => summary,
        Err(error) => return error.into_response(),
    };

    // Calculate yearly summary (last 365 days)
    let one_year_ago = today - Duration::days(365);
    let yearly_summary = match get_transaction_summary(one_year_ago..=today, &connection) {
        Ok(summary) => summary,
        Err(error) => return error.into_response(),
    };

    // Get total account balance
    let total_account_balance = match get_total_account_balance(&connection) {
        Ok(total) => total,
        Err(error) => return error.into_response(),
    };

    DashboardTemplate {
        nav_bar,
        monthly_summary,
        yearly_summary,
        total_account_balance,
    }
    .into_response()
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
        db::initialize,
        state::DashboardState,
        transaction::{Transaction, create_transaction},
    };
    use rusqlite::Connection;
    use std::sync::{Arc, Mutex};

    use super::get_dashboard_page;

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
        let monthly_section = get_section_by_heading(&html, "Last 30 Days");
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
}
