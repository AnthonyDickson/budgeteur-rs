//! JSON API handler for dashboard data. Returns a summary for the TUI client.
//!
//! All aggregations should ignore transactions with excluded tags.

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    ops::RangeInclusive,
};

use axum::{Json, extract::State, response::IntoResponse};
use time::{Date, Duration, Month, OffsetDateTime};

use crate::{
    Error,
    account::get_total_account_balance,
    dashboard::{
        handlers::DashboardState,
        transaction::{Transaction, get_transactions_in_date_range},
    },
    tag::get_excluded_tag_names,
    timezone::get_local_offset,
    transaction::get_untagged_transactions,
};
use budgeteur_shared::dashboard::{
    DashboardData, ExpensesByTagStats, NetIncomeStats, NetWorthStats, SavingsStats,
    SpendingPaceStats, UntaggedTransaction,
};

const UNTAGGED_TRANSACTION_LIMIT: usize = 20;

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

    let total_balance = get_total_account_balance(&connection)
        .inspect_err(|error| tracing::error!("Could not get total account balance: {error}"))?;

    let excluded_tag_names = get_excluded_tag_names(&connection)
        .inspect_err(|error| tracing::error!("could not get excluded tags: {error}"))?
        .into_iter()
        .map(|t| t.to_string())
        .collect::<HashSet<_>>();

    let transactions = get_transactions_in_date_range(date_range.clone(), None, &connection)
        .inspect_err(|error| tracing::error!("Could not get transactions: {error}"))?
        .into_iter()
        .filter(|t| !excluded_tag_names.contains(&t.tag))
        .collect::<Vec<_>>();

    let untagged_rows = get_untagged_transactions(UNTAGGED_TRANSACTION_LIMIT, &connection)
        .inspect_err(|error| tracing::error!("Could not get untagged transactions: {error}"))?;

    let untagged_transactions: Vec<UntaggedTransaction> = untagged_rows
        .into_iter()
        .map(|row| UntaggedTransaction {
            date: row.date,
            amount: row.amount,
            description: row.description,
        })
        .collect();

    let months = months_in_range(&date_range);
    let monthly_income = compute_net_income(&transactions, &months, today);
    let mean_monthly_expenses = mean_monthly_expenses(&transactions);

    Ok(Json(DashboardData {
        net_worth: compute_net_worth(total_balance, &transactions, &months),
        net_income: monthly_income.clone(),
        expenses_by_tag: compute_expenses_by_tag(&transactions, today),
        spending_pace: compute_spending_pace(&transactions, today, mean_monthly_expenses),
        savings: compute_savings(
            mean_monthly_expenses,
            total_balance,
            &monthly_income.monthly,
        ),
        untagged_transactions,
    }))
}

fn round_two_dp(n: f64) -> f64 {
    (n * 100.0).round() / 100.0
}

// ---------------------------------------------------------------------------
// Date Range
// ---------------------------------------------------------------------------

/// Twelve-month date range ending at `today`.
fn last_twelve_months(today: Date) -> RangeInclusive<Date> {
    let start = today
        .replace_day(1)
        .unwrap()
        .replace_month(today.month().previous())
        .unwrap()
        - Duration::days(365);

    start..=today
}

// ---------------------------------------------------------------------------
// Month Utilities
// ---------------------------------------------------------------------------

/// Returns the first day of the given date's month.
fn first_of_month(date: Date) -> Date {
    date.replace_day(1).unwrap()
}

/// Generates every first-of-month date within the given range, in order.
fn months_in_range(range: &RangeInclusive<Date>) -> Vec<Date> {
    let mut months = Vec::new();
    let mut current = first_of_month(*range.start());
    let end = first_of_month(*range.end());

    while current <= end {
        months.push(current);
        current = next_month(current);
    }

    months
}

/// Returns the first day of the month following `date`.
fn next_month(date: Date) -> Date {
    let next = date.month().next();
    if next == Month::January {
        Date::from_calendar_date(date.year() + 1, Month::January, 1).unwrap()
    } else {
        date.replace_month(next).unwrap()
    }
}

/// Returns the number of days in a given month.
fn days_in_month(year: i32, month: Month) -> u8 {
    match month {
        Month::January
        | Month::March
        | Month::May
        | Month::July
        | Month::August
        | Month::October
        | Month::December => 31,
        Month::April | Month::June | Month::September | Month::November => 30,
        Month::February => {
            if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 {
                29
            } else {
                28
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Transaction Grouping
// ---------------------------------------------------------------------------

/// Sums all transaction amounts (positive + negative) per month.
fn sum_by_month(transactions: &[Transaction]) -> BTreeMap<Date, f64> {
    let mut totals = BTreeMap::new();
    for t in transactions {
        *totals.entry(first_of_month(t.date)).or_insert(0.0) += t.amount;
    }
    totals
}

/// Calculate the mean monthly expenses.
///
/// Returns the absolute amount.
fn mean_monthly_expenses(transactions: &[Transaction]) -> f64 {
    let mut expenses_by_month = HashMap::new();
    for t in transactions.iter().filter(|t| t.amount < 0.0) {
        let date = first_of_month(t.date);
        let new_total = t.amount.abs() + expenses_by_month.get(&date).copied().unwrap_or(0.0);
        expenses_by_month.insert(date, new_total);
    }

    if !expenses_by_month.is_empty() {
        let total_expenses: f64 = expenses_by_month.values().copied().sum();
        total_expenses / expenses_by_month.len() as f64
    } else {
        0.0
    }
}

/// Looks up each month in the map, returning 0.0 for missing entries.
fn monthly_values_to_vec(monthly: &BTreeMap<Date, f64>, months: &[Date]) -> Vec<f64> {
    months
        .iter()
        .map(|m| monthly.get(m).copied().unwrap_or(0.0))
        .collect()
}

// ---------------------------------------------------------------------------
// Balance Reconstruction
// ---------------------------------------------------------------------------

/// Reconstruct month-end balances by working backwards from the current total,
/// then summing historical net flows in reverse.
fn reconstruct_monthly_balances(total_balance: f64, monthly_net: &[f64]) -> Vec<f64> {
    let mut balances = Vec::with_capacity(monthly_net.len());
    let mut cumulative = 0.0;
    for net in monthly_net.iter().rev() {
        balances.push((total_balance - cumulative).round());
        cumulative += net;
    }
    balances.reverse();
    balances
}

// ---------------------------------------------------------------------------
// Net Worth
// ---------------------------------------------------------------------------

fn compute_net_worth(
    total_balance: f64,
    transactions: &[Transaction],
    months: &[Date],
) -> NetWorthStats {
    let net_by_month = sum_by_month(transactions);
    let monthly_net = monthly_values_to_vec(&net_by_month, months);
    let balances = reconstruct_monthly_balances(total_balance, &monthly_net);

    let trend = if balances.len() >= 2 {
        balances.last().unwrap() - balances.first().unwrap()
    } else {
        0.0
    };

    NetWorthStats {
        amount: total_balance,
        trend,
        monthly: balances,
    }
}

// ---------------------------------------------------------------------------
// Net Income
// ---------------------------------------------------------------------------

fn compute_net_income(
    transactions: &[Transaction],
    months: &[Date],
    today: Date,
) -> NetIncomeStats {
    let cutoff = today - Duration::days(27);
    let last_28_days: f64 = transactions
        .iter()
        .filter(|t| t.date >= cutoff && t.date <= today)
        .map(|t| t.amount)
        .sum();

    let net_by_month = sum_by_month(transactions);
    let monthly = monthly_values_to_vec(&net_by_month, months)
        .iter()
        .map(|n| n.round())
        .collect::<Vec<f64>>();

    let monthly_avg = if monthly.is_empty() {
        0.0
    } else {
        monthly.iter().sum::<f64>() / monthly.len() as f64
    };

    NetIncomeStats {
        last_28_days,
        monthly_avg,
        monthly,
    }
}

// ---------------------------------------------------------------------------
// Expenses by Tag
// ---------------------------------------------------------------------------

fn compute_expenses_by_tag(transactions: &[Transaction], today: Date) -> Vec<ExpensesByTagStats> {
    let cutoff = today - Duration::days(27);
    let expenses: Vec<&Transaction> = transactions
        .iter()
        .filter(|t| t.date >= cutoff)
        .filter(|t| t.amount < 0.0)
        .collect();

    let total: f64 = expenses.iter().map(|t| t.amount.abs()).sum();

    if total == 0.0 {
        return Vec::new();
    }

    let mut by_tag: BTreeMap<&str, f64> = BTreeMap::new();
    for t in &expenses {
        *by_tag.entry(&t.tag).or_insert(0.0) += t.amount.abs();
    }

    let mut stats: Vec<ExpensesByTagStats> = by_tag
        .into_iter()
        .map(|(tag, amount)| ExpensesByTagStats {
            tag_name: tag.to_owned(),
            amount,
            ratio_of_expense: (amount / total),
        })
        .collect();

    stats.sort_by(|a, b| b.ratio_of_expense.total_cmp(&a.ratio_of_expense));

    stats
}

// ---------------------------------------------------------------------------
// Spending Pace
// ---------------------------------------------------------------------------

fn compute_spending_pace(
    transactions: &[Transaction],
    today: Date,
    mean_monthly_expenses: f64,
) -> SpendingPaceStats {
    let current_month = first_of_month(today);

    // Separate current-month expenses from historical ones.
    let (current_txns, historical_txns): (Vec<&Transaction>, Vec<&Transaction>) = transactions
        .iter()
        .filter(|t| t.amount < 0.0)
        .partition(|t| first_of_month(t.date) == current_month);

    let current_month_days = days_in_month(today.year(), today.month()) as usize;

    let historical: Vec<f64> = {
        let mut amounts_by_day: BTreeMap<usize, f64> = BTreeMap::new();
        let mut months_by_day: BTreeMap<usize, HashSet<(Month, i32)>> = BTreeMap::new();
        for t in &historical_txns {
            let day_idx = t.date.day() as usize - 1;
            *amounts_by_day.entry(day_idx).or_insert(0.0) += t.amount.abs();
            months_by_day
                .entry(day_idx)
                .or_default()
                .insert((t.date.month(), t.date.year()));
        }

        let mut historical = vec![0.0; current_month_days];
        for day_idx in 1..current_month_days {
            let total = amounts_by_day.get(&day_idx).copied().unwrap_or(0.0);
            let month_count = months_by_day.get(&day_idx).map(|s| s.len()).unwrap_or(0);
            let daily_avg = if month_count > 0 {
                total / month_count as f64
            } else {
                0.0
            };
            historical[day_idx] = round_two_dp(daily_avg + historical[day_idx - 1]);
        }

        historical
    };

    let (current, last_day_with_data) = {
        let mut sum_by_day = vec![0.0; current_month_days];
        let mut last_day_with_data = current_month;
        for t in current_txns {
            let day = (t.date.day() - 1) as usize;
            sum_by_day[day] = sum_by_day.get(day).copied().unwrap_or(0.0) + t.amount.abs();

            if t.date > last_day_with_data {
                last_day_with_data = t.date;
            }
        }

        let days_with_data = (last_day_with_data - current_month)
            .whole_days()
            .unsigned_abs() as usize
            + 1;
        let mut current = vec![0.0; days_with_data];
        for day in 0..days_with_data {
            let todays_total = sum_by_day[day];
            let Some(previous_day) = day.checked_sub(1) else {
                continue;
            };
            let previous_total = current.get(previous_day).copied().unwrap_or(0.0);
            current[day] = round_two_dp(todays_total + previous_total);
        }

        (current, last_day_with_data.day() as usize)
    };

    let baseline = historical[last_day_with_data - 1];
    let deviation_from_baseline = current[last_day_with_data - 1] - baseline;
    let deviation_from_baseline_ratio = if baseline != 0.0 {
        Some(deviation_from_baseline / baseline)
    } else {
        None
    };

    SpendingPaceStats {
        historical,
        current,
        deviation_from_baseline,
        deviation_from_baseline_ratio,
        mean_monthly_expenses,
    }
}

// ---------------------------------------------------------------------------
// Savings
// ---------------------------------------------------------------------------

fn compute_savings(
    mean_monthly_expenses: f64,
    total_balance: f64,
    monthly_net: &[f64],
) -> SavingsStats {
    let balances = reconstruct_monthly_balances(total_balance, monthly_net);

    // Trend: change over the last 12 months including the current month
    let trend = balances
        .last()
        .copied()
        .map(|last_balance| last_balance - balances[0])
        .unwrap_or(0.0);

    let months_of_savings = if mean_monthly_expenses > 0.0 {
        (total_balance / mean_monthly_expenses) as u64
    } else {
        0
    };

    SavingsStats {
        amount: total_balance,
        trend,
        months_of_savings,
        monthly: balances,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use axum::{extract::State, response::IntoResponse};
    use rusqlite::Connection;
    use time::macros::date;

    use crate::{
        dashboard::{
            handlers::DashboardState,
            json::{
                compute_expenses_by_tag, compute_net_income, compute_net_worth, compute_savings,
                compute_spending_pace, first_of_month, get_dashboard_json, last_twelve_months,
                monthly_values_to_vec, months_in_range, sum_by_month,
            },
            transaction::Transaction,
        },
        db::initialize,
    };
    use time::Date;

    // -----------------------------------------------------------------------
    // Test helpers
    // -----------------------------------------------------------------------

    fn build_state() -> DashboardState {
        let conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();
        DashboardState {
            db_connection: std::sync::Arc::new(std::sync::Mutex::new(conn)),
            local_timezone: "Etc/UTC".into(),
        }
    }

    fn tx(amount: f64, date: Date, tag: &str) -> Transaction {
        Transaction {
            amount,
            date,
            tag: tag.to_owned(),
        }
    }

    // -----------------------------------------------------------------------
    // Handler test
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn returns_json_with_empty_db() {
        // Given: an empty database
        let state = build_state();

        // When: calling get_dashboard_json
        let response = get_dashboard_json(State(state))
            .await
            .unwrap()
            .into_response();

        // Then: returns 200 with zeroed dashboard data
        assert_eq!(response.status(), 200);
        let body = axum::body::to_bytes(response.into_body(), 32768)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["net_worth"]["amount"], 0.0);
        assert_eq!(json["net_worth"]["trend"], 0.0);
        let net_worth_monthly = json["net_worth"]["monthly"].as_array().unwrap();
        assert!(!net_worth_monthly.is_empty());
        assert!(net_worth_monthly.iter().all(|v| v.as_f64() == Some(0.0)));
        assert_eq!(json["net_income"]["last_28_days"], 0.0);
        assert_eq!(json["net_income"]["monthly_avg"], 0.0);
        assert_eq!(json["expenses_by_tag"].as_array().unwrap().len(), 0);
        assert_eq!(json["untagged_transactions"].as_array().unwrap().len(), 0);
    }

    // -----------------------------------------------------------------------
    // Month utility tests
    // -----------------------------------------------------------------------

    #[test]
    fn first_of_month_returns_day_one() {
        // Given: two dates, one mid-month, one already on the first
        // When: calling first_of_month on each
        // Then: both return the first day of their respective month
        assert_eq!(first_of_month(date!(2024 - 03 - 15)), date!(2024 - 03 - 01));
        assert_eq!(first_of_month(date!(2024 - 01 - 01)), date!(2024 - 01 - 01));
    }

    #[test]
    fn months_in_range_spans_years() {
        // Given: a range crossing a year boundary (Nov 2024 to Feb 2025)
        let range = date!(2024 - 11 - 15)..=date!(2025 - 02 - 10);

        // When: calling months_in_range
        let result = months_in_range(&range);

        // Then: returns first-of-month dates for all four months
        assert_eq!(
            result,
            vec![
                date!(2024 - 11 - 01),
                date!(2024 - 12 - 01),
                date!(2025 - 01 - 01),
                date!(2025 - 02 - 01),
            ]
        );
    }

    #[test]
    fn months_in_range_single_month() {
        // Given: a range within a single month
        let range = date!(2024 - 06 - 10)..=date!(2024 - 06 - 25);

        // When: calling months_in_range
        let result = months_in_range(&range);

        // Then: returns just that one month's first day
        assert_eq!(result, vec![date!(2024 - 06 - 01)]);
    }

    #[test]
    fn last_twelve_months_covers_roughly_one_year() {
        // Given: today is May 18, 2026
        let range = last_twelve_months(date!(2026 - 05 - 18));

        // When: calling last_twelve_months
        let months = months_in_range(&range);

        // Then: the range spans at least 12 distinct months
        assert!(months.len() >= 12, "got {} months", months.len());
    }

    // -----------------------------------------------------------------------
    // Grouping tests
    // -----------------------------------------------------------------------

    #[test]
    fn sum_by_month_aggregates_correctly() {
        // Given: transactions in Jan (+100, +50) and Feb (-30)
        let transactions = vec![
            tx(100.0, date!(2024 - 01 - 05), "A"),
            tx(50.0, date!(2024 - 01 - 20), "B"),
            tx(-30.0, date!(2024 - 02 - 10), "C"),
        ];

        // When: calling sum_by_month
        let result = sum_by_month(&transactions);

        // Then: Jan totals 150, Feb totals -30
        assert_eq!(result[&date!(2024 - 01 - 01)], 150.0);
        assert_eq!(result[&date!(2024 - 02 - 01)], -30.0);
    }

    #[test]
    fn monthly_values_to_vec_fills_gaps_with_zero() {
        // Given: a sparse BTreeMap with Jan and Mar entries, Feb missing
        let mut map = BTreeMap::new();
        map.insert(date!(2024 - 01 - 01), 100.0);
        map.insert(date!(2024 - 03 - 01), 200.0);
        let months = vec![
            date!(2024 - 01 - 01),
            date!(2024 - 02 - 01),
            date!(2024 - 03 - 01),
        ];

        // When: calling monthly_values_to_vec
        let result = monthly_values_to_vec(&map, &months);

        // Then: returns [100, 0, 200] — Feb gap filled with 0
        assert_eq!(result, vec![100.0, 0.0, 200.0]);
    }

    // -----------------------------------------------------------------------
    // Net worth tests
    // -----------------------------------------------------------------------

    #[test]
    fn net_worth_reconstructs_balances() {
        // Given: monthly net flows of +100, -50, +200 and total balance 250
        let transactions = vec![
            tx(100.0, date!(2024 - 01 - 10), "A"),
            tx(-50.0, date!(2024 - 02 - 15), "B"),
            tx(200.0, date!(2024 - 03 - 20), "C"),
        ];
        let months = vec![
            date!(2024 - 01 - 01),
            date!(2024 - 02 - 01),
            date!(2024 - 03 - 01),
        ];

        // When: calling compute_net_worth
        let result = compute_net_worth(250.0, &transactions, &months);

        // Then: reconstructs month-end balances [100, 50, 250], trend 150
        // Monthly net: Jan +100, Feb -50, Mar +200 → balance started at 0,
        // so current total = 100 - 50 + 200 = 250.
        assert_eq!(result.amount, 250.0);
        // End of Jan: 0 + 100 = 100, End of Feb: 100 - 50 = 50, End of Mar: 50 + 200 = 250
        assert_eq!(result.monthly, vec![100.0, 50.0, 250.0]);
        // Trend over all months
        assert_eq!(result.trend, 150.0);
    }

    #[test]
    fn net_worth_empty_transactions() {
        // Given: two months with no transactions, starting balance 500
        let months = vec![date!(2024 - 01 - 01), date!(2024 - 02 - 01)];

        // When: calling compute_net_worth
        let result = compute_net_worth(500.0, &[], &months);

        // Then: amount stays 500, monthly flat at [500, 500], trend 0
        assert_eq!(result.amount, 500.0);
        assert_eq!(result.monthly, vec![500.0, 500.0]);
        assert_eq!(result.trend, 0.0);
    }

    #[test]
    fn net_worth_single_month() {
        // Given: one month with +100 flow, balance 100
        let transactions = vec![tx(100.0, date!(2024 - 06 - 10), "A")];
        let months = vec![date!(2024 - 06 - 01)];

        // When: calling compute_net_worth
        let result = compute_net_worth(100.0, &transactions, &months);

        // Then: monthly is [100], trend is 0 (not enough data)
        assert_eq!(result.monthly, vec![100.0]);
        assert_eq!(result.trend, 0.0);
    }

    // -----------------------------------------------------------------------
    // Net income tests
    // -----------------------------------------------------------------------

    #[test]
    fn net_income_computes_last_28_days() {
        // Given: today is Feb 15, transactions: +100 (Feb 14), -30 (Feb 1), +50 (Jan 10 — too old)
        let today = date!(2024 - 02 - 15);
        let transactions = vec![
            tx(100.0, date!(2024 - 02 - 14), "A"),
            tx(-30.0, date!(2024 - 02 - 01), "B"),
            tx(50.0, date!(2024 - 01 - 10), "C"), // too old
        ];
        let months = vec![date!(2024 - 01 - 01), date!(2024 - 02 - 01)];

        // When: calling compute_net_income
        let result = compute_net_income(&transactions, &months, today);

        // Then: last_28_days = 100 - 30 = 70
        assert_eq!(result.last_28_days, 70.0); // 100 - 30
    }

    #[test]
    fn net_income_monthly_and_avg() {
        // Given: Jan net 50, Feb net 200
        // When: calling compute_net_income
        // Then: monthly = [50, 200], avg = 125
        let transactions = vec![
            tx(100.0, date!(2024 - 01 - 10), "A"),
            tx(-50.0, date!(2024 - 01 - 20), "B"),
            tx(200.0, date!(2024 - 02 - 05), "C"),
        ];
        let months = vec![date!(2024 - 01 - 01), date!(2024 - 02 - 01)];
        let result = compute_net_income(&transactions, &months, date!(2024 - 02 - 15));
        assert_eq!(result.monthly, vec![50.0, 200.0]);
        assert_eq!(result.monthly_avg, 125.0);
    }

    #[test]
    fn net_income_empty_returns_zeros() {
        // Given: no transactions, no months
        let months: Vec<Date> = vec![];

        // When: calling compute_net_income
        let result = compute_net_income(&[], &months, date!(2024 - 01 - 01));

        // Then: all values are zero
        assert_eq!(result.last_28_days, 0.0);
        assert_eq!(result.monthly_avg, 0.0);
        assert!(result.monthly.is_empty());
    }

    // -----------------------------------------------------------------------
    // Expenses by tag tests
    // -----------------------------------------------------------------------

    #[test]
    fn expenses_by_tag_uses_27_day_window() {
        // Given: today = Mar 15 → cutoff = Feb 17 (27 days back)
        let transactions = vec![
            tx(-60.0, date!(2024 - 02 - 17), "Food"),
            tx(-40.0, date!(2024 - 02 - 20), "Transport"),
            tx(-100.0, date!(2024 - 03 - 05), "Food"),
            tx(50.0, date!(2024 - 02 - 10), "Income"), // positive, excluded
        ];

        // When: calling compute_expenses_by_tag
        let result = compute_expenses_by_tag(&transactions, date!(2024 - 03 - 15));

        // Then: aggregates expenses in the 27-day window, sorted by ratio descending
        // today = Mar 15 → cutoff = Feb 17 (27 days back)
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].tag_name, "Food");
        assert_eq!(result[0].amount, 160.0);
        assert_eq!(result[0].ratio_of_expense, 0.8);
        assert_eq!(result[1].tag_name, "Transport");
        assert_eq!(result[1].amount, 40.0);
        assert_eq!(result[1].ratio_of_expense, 0.2);
    }

    #[test]
    fn expenses_by_tag_empty_when_no_expenses() {
        // Given: only a positive (income) transaction in the window
        let transactions = vec![tx(100.0, date!(2024 - 02 - 10), "Income")];

        // When: calling compute_expenses_by_tag
        let result = compute_expenses_by_tag(&transactions, date!(2024 - 03 - 15));

        // Then: result is empty
        assert!(result.is_empty());
    }

    #[test]
    fn expenses_by_tag_sorted_by_amount_descending() {
        // Given: two expenses: Small ($30) and Large ($70)
        let transactions = vec![
            tx(-30.0, date!(2024 - 02 - 18), "Small"),
            tx(-70.0, date!(2024 - 02 - 20), "Large"),
        ];

        // When: calling compute_expenses_by_tag
        let result = compute_expenses_by_tag(&transactions, date!(2024 - 03 - 15));

        // Then: result is sorted by amount descending (Large first)
        assert_eq!(result[0].tag_name, "Large");
        assert_eq!(result[1].tag_name, "Small");
    }

    // -----------------------------------------------------------------------
    // Spending pace tests
    // -----------------------------------------------------------------------

    #[test]
    fn spending_pace_historical_averages_across_months() {
        // Given: two historical months each with $10/day, current month with $15 on day 1
        let transactions = vec![
            tx(-10.0, date!(2024 - 01 - 01), "A"),
            tx(-10.0, date!(2024 - 01 - 02), "A"),
            tx(-10.0, date!(2024 - 02 - 01), "A"),
            tx(-10.0, date!(2024 - 02 - 02), "A"),
            // Current month (Mar): only day 1
            tx(-15.0, date!(2024 - 03 - 01), "A"),
        ];

        // When: calling compute_spending_pace (Mar 2, mean_monthly_expenses = 20)
        let result = compute_spending_pace(&transactions, date!(2024 - 03 - 02), 20.0);

        // Then: deviates by 0 at day 1 (both start at 0), historical[1] = 10 avg
        // Two historical months, each with $10/day on day 1 and 2.
        // Historical average day 1: $10, day 2: $20 (cumulative).
        assert_eq!(result.historical.len(), 31);
        assert_eq!(result.current.len(), 1);
        assert_eq!(result.historical[0], 0.0);
        assert_eq!(result.historical[1], 10.0);
        assert_eq!(result.current[0], 0.0);
        assert_eq!(result.deviation_from_baseline, 0.0);
        assert!(result.deviation_from_baseline_ratio.is_none());
    }

    #[test]
    fn spending_pace_no_historical_returns_zeros() {
        // Given: only current-month transactions, no history, mean = 0
        let transactions = vec![tx(-50.0, date!(2024 - 03 - 10), "A")];

        // When: calling compute_spending_pace
        let result = compute_spending_pace(&transactions, date!(2024 - 03 - 15), 0.0);

        // Then: historical is all zeros, deviation positive, ratio infinite (no baseline)
        // Only current month transactions, no history
        assert!(result.historical.iter().all(|&x| x == 0.0));
        assert!(result.deviation_from_baseline > 0.0);
        assert!(result.deviation_from_baseline_ratio.is_none());
    }

    // -----------------------------------------------------------------------
    // Savings tests
    // -----------------------------------------------------------------------

    #[test]
    fn savings_reconstructs_balances() {
        // Given: 3 months of net flows [100, -50, 200], total balance 250, mean expenses 125
        let monthly_net = vec![100.0, -50.0, 200.0];

        // When: calling compute_savings
        let result = compute_savings(125.0, 250.0, &monthly_net);

        // Then: balances [100, 50, 250], trend 150, 2 months of savings
        assert_eq!(result.amount, 250.0);
        assert_eq!(result.monthly, vec![100.0, 50.0, 250.0]);
        assert_eq!(result.trend, 150.0);
        assert_eq!(result.months_of_savings, 2); // 250 / 125
    }

    #[test]
    fn savings_months_of_savings_zero_when_negative_income() {
        // Given: negative mean monthly expenses (-50), balance 1000
        // When: calling compute_savings
        let result = compute_savings(-50.0, 1000.0, &[100.0]);

        // Then: months_of_savings is 0 (expenses must be positive to divide)
        assert_eq!(result.months_of_savings, 0);
    }
}
