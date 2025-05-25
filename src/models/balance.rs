//! Defines the model for account balances.

/// The amount of money available for a bank account or credit card.
#[derive(Debug, Clone)]
pub struct Balance {
    /// The account with which to associate the balance.
    pub account: String,
    /// The balance.
    pub balance: f64,
}
