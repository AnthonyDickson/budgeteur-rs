//! Defines the store for bank/credit card account balances.

use time::Date;

use crate::{
    Error,
    models::{Balance, UserID},
};

/// Handles the creation and retrieval of account balances.
pub trait BalanceStore {
    /// Create a new account balance in the store.
    fn create(&mut self, account: &str, balance: f64, date: &Date) -> Result<Balance, Error>;

    /// Retrieve all balances for a given `user_id` from the store.
    fn get_by_user_id(&self, user_id: UserID) -> Result<Vec<Balance>, Error>;
}
