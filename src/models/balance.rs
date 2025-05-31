//! Defines the model for account balances.

use super::{DatabaseID, UserID};

/// The amount of money available for a bank account or credit card.
#[derive(Debug, Clone, PartialEq)]
pub struct Balance {
    /// The id for the account balance.
    pub id: DatabaseID,
    /// The account with which to associate the balance.
    pub account: String,
    /// The balance.
    pub balance: f64,
    /// The id of the user this balance belong to.
    pub user_id: UserID,
}
