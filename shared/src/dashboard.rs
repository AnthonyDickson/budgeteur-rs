use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardSummary {
    pub total_balance: f64,
    pub monthly_income: f64,
    pub monthly_expenses: f64,
    pub monthly_net: f64,
}
