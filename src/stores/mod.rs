//! Contains traits and implementations for objects that store the domain [models](crate::models).

// TODO: Create table for bank/credit card accounts
// TODO: Create table for account balances
pub mod category;
pub mod sql_store;
pub mod transaction;
pub mod user;

pub use category::{CategoryStore, SQLiteCategoryStore};
pub use transaction::{SQLiteTransactionStore, TransactionStore};
pub use user::{SQLiteUserStore, UserStore};
