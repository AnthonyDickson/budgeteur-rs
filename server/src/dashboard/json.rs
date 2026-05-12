//! JSON API handler for dashboard data. Returns a summary for the TUI client.

use axum::{Json, extract::State, response::IntoResponse};
use time::{Date, Duration, OffsetDateTime};

use crate::{
    Error,
    account::get_total_account_balance,
    dashboard::{
        handlers::DashboardState,
        transaction::{Transaction, get_transactions_in_date_range},
    },
    timezone::get_local_offset,
};

pub use budgeteur_shared::dashboard::DashboardSummary;

/// Return a JSON summary of the dashboard for the TUI.
pub async fn get_dashboard_json(
    State(state): State<DashboardState>,
) -> Result<impl IntoResponse, Error> {
    let connection = state
        .db_connection
        .lock()
        .inspect_err(|error| tracing::error!("could not acquire database lock: {error}"))
        .map_err(|_| Error::DatabaseLockError)?;

    let offset = get_local_offset(&state.local_timezone).ok_or_else(|| {
        tracing::error!("Invalid timezone {}", state.local_timezone);
        Error::InvalidTimezoneError(state.local_timezone.clone())
    })?;
    let today = OffsetDateTime::now_utc().to_offset(offset).date();
    let date_range = last_twelve_months(today);

    let transactions = get_transactions_in_date_range(date_range, None, &connection)
        .inspect_err(|error| tracing::error!("Could not get transactions: {error}"))?;

    let total_balance = get_total_account_balance(&connection).inspect_err(|error| {
        tracing::error!("Could not calculate total account balance: {error}")
    })?;

    let monthly = last_month_summary(&transactions, today);

    Ok(Json(DashboardSummary {
        total_balance,
        monthly_income: monthly.income,
        monthly_expenses: monthly.expenses,
        monthly_net: monthly.income - monthly.expenses,
    }))
}

/// Simple income/expense totals for a single month.
struct MonthlyTotals {
    income: f64,
    expenses: f64,
}

/// Calculate income and expenses for the last complete month.
fn last_month_summary(transactions: &[Transaction], today: Date) -> MonthlyTotals {
    let last_complete_month = today.replace_day(1).unwrap() - Duration::days(1);
    let month_start = last_complete_month.replace_day(1).unwrap();

    let mut income = 0.0;
    let mut expenses = 0.0;

    for t in transactions {
        if t.date >= month_start && t.date <= last_complete_month {
            if t.amount >= 0.0 {
                income += t.amount;
            } else {
                expenses += t.amount.abs();
            }
        }
    }

    MonthlyTotals { income, expenses }
}

/// Twelve-month date range ending at `today`.
fn last_twelve_months(today: Date) -> std::ops::RangeInclusive<Date> {
    let start = today
        .replace_day(1)
        .unwrap()
        .replace_month(today.month().previous())
        .unwrap()
        - time::Duration::days(365);

    start..=today
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use axum::{extract::State, response::IntoResponse};
    use rusqlite::Connection;

    use crate::{
        dashboard::{handlers::DashboardState, json::get_dashboard_json},
        db::initialize,
    };

    fn build_state() -> DashboardState {
        let conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();
        DashboardState {
            db_connection: std::sync::Arc::new(std::sync::Mutex::new(conn)),
            local_timezone: "Etc/UTC".into(),
        }
    }

    #[tokio::test]
    async fn returns_json_with_empty_db() {
        // Given an empty database
        let state = build_state();

        // When the dashboard JSON endpoint is called
        let response = get_dashboard_json(State(state))
            .await
            .unwrap()
            .into_response();

        // Then it returns 200 with zeroed summary fields
        assert_eq!(response.status(), 200);
        let body = axum::body::to_bytes(response.into_body(), 1024)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["total_balance"], 0.0);
        assert_eq!(json["monthly_income"], 0.0);
        assert_eq!(json["monthly_expenses"], 0.0);
        assert_eq!(json["monthly_net"], 0.0);
    }
}
