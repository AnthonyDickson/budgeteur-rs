//! Defines the store for bank/credit card account balances.

use time::Date;

use crate::{Error, models::Balance};

/// Handles the creation and retrieval of account balances.
pub trait BalanceStore {
    /// Create a new account balance in the store, or update the entry with
    /// the same account if it exists.
    fn upsert(&mut self, account: &str, balance: f64, date: &Date) -> Result<Balance, Error>;

    /// Retrieve all balances from the store.
    fn get_all(&self) -> Result<Vec<Balance>, Error>;
}
