//! The types shared between the server and the TUI client for the dashboard view.

use serde::{Deserialize, Serialize};
use time::Date;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetWorthStats {
    /// The current net worth
    pub amount: f64,
    /// The change in net worth over the trailing 12 months
    pub trend: f64,
    /// Net worth at the end of each month, trailing 12 months.
    /// For the current month, this will be the current net worth.
    pub monthly: Vec<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetIncomeStats {
    /// Net income for the last 28 days
    pub last_28_days: f64,
    /// Mean monthly net income, TTM
    pub monthly_avg: f64,
    /// Net income per month, trailing 12 months
    pub monthly: Vec<f64>,
}

/// The expenses for a given tag over the last 28 days.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpensesByTagStats {
    /// The tag name.
    pub tag_name: String,
    /// The total expenses for the last 28 days.
    pub amount: f64,
    /// The ratio of the expenses for this tag against the total expenses for the last 28 days
    pub ratio_of_expense: f64,
}

/// The trajectory of the current month's spending compared to historical trends
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpendingPaceStats {
    /// Historical mean cumulative expenditures by day of month, calculated from the TTM of data.
    pub historical: Vec<f64>,
    /// Current month cumulative expenditures by day of month.
    pub current: Vec<f64>,
    /// The expected difference between the end of month expenses.
    ///
    /// Calculated as the difference between the current month cumulative
    /// expenses minus the mean historical cumulative expenses for the corresponding day.
    /// This assumes the you will continue to spend at your typical rate.
    pub deviation_from_baseline: f64,
    /// The expected difference between the end of month expenses as a ratio.
    ///
    /// `None` indicates that the baseline is not meaningful, e.g. it is the
    /// start of the month where the baseline is zero, and the current spending is zero.
    pub deviation_from_baseline_ratio: Option<f64>,
    /// The mean monthly expentiture as an absolute value.
    pub mean_monthly_expenses: f64,
}

/// Snapshot of the savings, runway and trend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavingsStats {
    /// The total savings (liquid net worth)
    pub amount: f64,
    /// How much savings have changed in the trailing 12 weeks (3 months)
    pub trend: f64,
    /// How many months would the current savings last at the the monthly net income calculated over the last 12 months
    pub months_of_savings: u64,
    /// The savings at the end of each month.
    /// For the current month, this will be the current savings.
    pub monthly: Vec<f64>,
}

/// Imported transactions that were not auto-tagged.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UntaggedTransaction {
    pub date: Date,
    pub amount: f64,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardData {
    pub net_worth: NetWorthStats,
    pub net_income: NetIncomeStats,
    pub expenses_by_tag: Vec<ExpensesByTagStats>,
    pub spending_pace: SpendingPaceStats,
    pub savings: SavingsStats,
    pub untagged_transactions: Vec<UntaggedTransaction>,
}
