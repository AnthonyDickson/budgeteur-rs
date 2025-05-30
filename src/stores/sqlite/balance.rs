//! Implements a SQLite backed balance store.
use crate::{
    Error,
    models::{Balance, DatabaseID, UserID},
    stores::BalanceStore,
};

// TODO: Implement SQLite store for balances.
/// Create and retrieve account balances.
#[derive(Debug, Clone)]
pub struct StubBalanceStore;

impl BalanceStore for StubBalanceStore {
    fn create(&mut self, _account: &str, _balancee: f64) -> Result<Balance, Error> {
        todo!()
    }

    fn get(&self, _id: DatabaseID) -> Result<Balance, Error> {
        todo!()
    }

    fn get_by_user_id(&self, _user_id: UserID) -> Result<Vec<Balance>, Error> {
        Ok(vec![])
    }
}
