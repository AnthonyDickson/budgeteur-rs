//! Transaction data aggregation and transformation for charts.
//!
//! Provides functions to aggregate transaction data by month, calculate running
//! balances, group expenses by tag, and format data for chart display.

use std::collections::{HashMap, HashSet};

use time::Date;

use crate::dashboard::transaction::{Transaction, UNTAGGED_LABEL};

/// Aggregates transaction amounts by month.
///
/// # Returns
/// HashMap mapping each month (as Date with day=1) to the sum of transaction amounts.
pub(super) fn aggregate_by_month(transactions: &[Transaction]) -> HashMap<Date, f64> {
    let mut totals = HashMap::new();

    for transaction in transactions {
        let month = transaction.date.replace_day(1).unwrap();
        *totals.entry(month).or_insert(0.0) += transaction.amount;
    }

    totals
}

/// Extracts unique months from transactions and returns them in chronological order.
///
/// # Returns
/// Vector of unique months (as Dates with day=1) sorted chronologically.
pub(super) fn get_sorted_months(transactions: &[Transaction]) -> Vec<Date> {
    let mut months = HashSet::new();

    for transaction in transactions {
        let month = transaction.date.replace_day(1).unwrap();
        months.insert(month);
    }

    let mut sorted: Vec<_> = months.into_iter().collect();
    sorted.sort();
    sorted
}

/// Formats month dates as three-letter abbreviations.
///
/// # Arguments
/// * `months` - Vector of dates to format
///
/// # Returns
/// Vector of month names as 3-letter strings (e.g., "Jan", "Feb").
pub(super) fn format_month_labels(months: &[Date]) -> Vec<String> {
    use time::Month;
    let month_to_str = |date: &Date| {
        match date.month() {
            Month::January => "Jan",
            Month::February => "Feb",
            Month::March => "Mar",
            Month::April => "Apr",
            Month::May => "May",
            Month::June => "Jun",
            Month::July => "Jul",
            Month::August => "Aug",
            Month::September => "Sep",
            Month::October => "Oct",
            Month::November => "Nov",
            Month::December => "Dec",
        }
        .to_string()
    };

    months.iter().map(month_to_str).collect()
}

/// Converts monthly aggregate data into sorted labels and values for charting.
///
/// # Arguments
/// * `monthly_totals` - HashMap of months to their aggregated transaction amounts
///
/// # Returns
/// Tuple of (month labels as 3-letter abbreviations, corresponding values).
pub(super) fn get_monthly_label_and_value_pairs(
    monthly_totals: &HashMap<Date, f64>,
) -> (Vec<String>, Vec<f64>) {
    let mut sorted_months: Vec<Date> = monthly_totals.keys().copied().collect();
    sorted_months.sort();

    let labels = format_month_labels(&sorted_months);
    let values = sorted_months
        .iter()
        .map(|month| monthly_totals[month])
        .collect();

    (labels, values)
}

/// Calculates running account balances by working backwards from the current total.
///
/// # Arguments
/// * `total_balance` - The current total account balance
/// * `monthly_totals` - HashMap of months to their net transaction amounts
///
/// # Returns
/// Tuple of (month labels as 3-letter abbreviations, corresponding balance values).
pub(super) fn calculate_running_balances(
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

/// Groups expense transactions by tag and calculates monthly totals.
///
/// Only negative amounts (expenses) are included. Returns data in a format
/// suitable for ECharts stacked bar charts, with "Other" tag sorted last.
///
/// # Arguments
/// * `transactions` - All transactions to analyze
/// * `sorted_months` - The months to include in the output (determines chart x-axis)
///
/// # Returns
/// Vector of (tag_name, monthly_values) tuples where monthly_values has one
/// entry per month in `sorted_months`, with `None` for months with no expenses.
pub(super) fn group_monthly_expenses_by_tag(
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
        .filter(|&tag| tag != UNTAGGED_LABEL)
        .collect();
    sorted_tags.sort();

    if transactions_by_tag.contains_key(UNTAGGED_LABEL) {
        sorted_tags.push(UNTAGGED_LABEL);
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

#[cfg(test)]
mod tests {
    use time::macros::date;

    use crate::dashboard::{
        aggregation::{
            aggregate_by_month, calculate_monthly_expenses, format_month_labels, get_sorted_months,
            group_monthly_expenses_by_tag,
        },
        transaction::{Transaction, UNTAGGED_LABEL},
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
            create_test_transaction(25.0, date!(2024 - 01 - 25), UNTAGGED_LABEL), // Same month as second
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
            create_test_transaction(-50.0, date!(2024 - 01 - 20), UNTAGGED_LABEL),
            create_test_transaction(-30.0, date!(2024 - 01 - 10), "Alpha"),
        ];

        let months = vec![date!(2024 - 01 - 01)];

        let result = group_monthly_expenses_by_tag(&transactions, &months);

        assert_eq!(result.len(), 3);
        // Check that "Other" is last
        assert_eq!(result[2].0, UNTAGGED_LABEL);
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
        assert!(result.iter().all(|(tag, _)| tag != UNTAGGED_LABEL));
    }
}
