//! Defines the transaction store trait.

use std::ops::RangeInclusive;

use time::Date;

use crate::{
    Error,
    models::{DatabaseID, Transaction, TransactionBuilder},
};

/// Handles the creation and retrieval of transactions.
pub trait TransactionStore {
    /// Create a new transaction in the store.
    fn create(&mut self, amount: f64) -> Result<Transaction, Error>;

    /// Create a new transaction in the store.
    fn create_from_builder(&mut self, builder: TransactionBuilder) -> Result<Transaction, Error>;

    /// Import many transactions from a CSV file.
    ///
    /// Implementers should ignore transactions with import IDs that already
    /// exist in the store.
    fn import(&mut self, builders: Vec<TransactionBuilder>) -> Result<Vec<Transaction>, Error>;

    /// Retrieve a transaction from the store.
    fn get(&self, id: DatabaseID) -> Result<Transaction, Error>;

    /// Retrieve transactions from the store in the way defined by `query`.
    fn get_query(&self, query: TransactionQuery) -> Result<Vec<Transaction>, Error>;
}

/// Defines how transactions should be fetched from [TransactionStore::get_query].
#[derive(Default)]
pub struct TransactionQuery {
    /// Include transactions within `date_range` (inclusive).
    pub date_range: Option<RangeInclusive<Date>>,
    /// Selects up to the first N (`limit`) transactions.
    pub limit: Option<u64>,
    /// Orders transactions by date in the order `sort_date`. None returns transactions in the
    /// order they are stored.
    pub sort_date: Option<SortOrder>,
}

/// The order to sort transactions in a [TransactionQuery].
pub enum SortOrder {
    /// Sort in order of increasing value.
    Ascending,
    /// Sort in order of decreasing value.
    Descending,
}
