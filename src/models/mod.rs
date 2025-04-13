//! This module defines the domain data types.

pub use category::{Category, CategoryName};
pub use password::{PasswordHash, ValidatedPassword};
pub use transaction::{Transaction, TransactionBuilder};
pub use user::{User, UserID};

// TODO: Add model for account numbers and balances.
mod category;
mod password;
mod transaction;
mod user;

/// Alias for the integer type used for mapping to database IDs.
pub type DatabaseID = i64;
