//! This file defines the dashboard route and its handlers.

use askama_axum::Template;
use axum::{
    extract::State,
    response::{IntoResponse, Response},
};
use time::{Duration, OffsetDateTime};

use crate::{
    routes::{
        endpoints,
        navigation::{NavbarTemplate, get_nav_bar},
    },
    state::DashboardState,
    transaction::{TransactionQuery, query_transactions},
};

/// Renders the dashboard page.
#[derive(Template)]
#[template(path = "views/dashboard.html")]
struct DashboardTemplate<'a> {
    nav_bar: NavbarTemplate<'a>,
    /// How much over or under budget the user is for this week.
    balance: f64,
}

/// Display a page with an overview of the user's data.
pub async fn get_dashboard_page(State(state): State<DashboardState>) -> Response {
    let nav_bar = get_nav_bar(endpoints::DASHBOARD_VIEW);

    let today = OffsetDateTime::now_utc().date();
    let one_week_ago = match today.checked_sub(Duration::weeks(1)) {
        Some(date) => date,
        None => {
            tracing::warn!(
                "Could not get date for one week before {today}. Using today's date ({today}) instead."
            );

            today
        }
    };

    let connection = state.db_connection.lock().unwrap();
    let transactions = query_transactions(
        TransactionQuery {
            date_range: Some(one_week_ago..=today),
            ..Default::default()
        },
        &connection,
    );

    let balance = match transactions {
        Ok(transactions) => transactions
            .iter()
            .map(|transaction| transaction.amount())
            .sum(),
        Err(error) => return error.into_response(),
    };

    DashboardTemplate { nav_bar, balance }.into_response()
}

#[cfg(test)]
mod dashboard_route_tests {
    use axum::{
        body::Body,
        extract::State,
        http::{Response, StatusCode},
    };
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
    async fn dashboard_displays_correct_balance() {
        let conn = get_test_connection();

        // Create transactions in the database
        // Transaction before the current week should not be included in the balance
        create_transaction(
            Transaction::build(12.3)
                .date(
                    OffsetDateTime::now_utc()
                        .date()
                        .checked_sub(Duration::weeks(2))
                        .unwrap(),
                )
                .unwrap(),
            &conn,
        )
        .unwrap();

        // These transactions should be included
        create_transaction(Transaction::build(45.6), &conn).unwrap();
        create_transaction(Transaction::build(-45.6), &conn).unwrap();
        create_transaction(Transaction::build(123.0), &conn).unwrap();

        let state = DashboardState {
            db_connection: Arc::new(Mutex::new(conn)),
        };

        let response = get_dashboard_page(State(state)).await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_body_contains_amount(response, "$123").await;
    }

    #[tokio::test]
    async fn dashboard_displays_negative_balance_without_sign() {
        let conn = get_test_connection();

        // Create transaction in the database
        create_transaction(Transaction::build(-123.0), &conn).unwrap();

        let state = DashboardState {
            db_connection: Arc::new(Mutex::new(conn)),
        };

        let response = get_dashboard_page(State(state)).await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_body_contains_amount(response, "$123").await;
    }

    async fn assert_body_contains_amount(response: Response<Body>, want: &str) {
        let body = response.into_body();
        let body = axum::body::to_bytes(body, usize::MAX).await.unwrap();

        let text = String::from_utf8_lossy(&body).to_string();

        assert!(
            text.contains(want),
            "response body should contain '{}' but got {}",
            want,
            text
        );
    }
}
