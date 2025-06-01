//! Defines the model for account balances.

use time::Date;

use super::DatabaseID;

/// The amount of money available for a bank account or credit card.
#[derive(Debug, Clone, PartialEq)]
pub struct Balance {
    /// The id for the account balance.
    pub id: DatabaseID,
    /// The account with which to associate the balance.
    pub account: String,
    /// The balance.
    pub balance: f64,
    /// When the balance was updated.
    pub date: Date,
}
