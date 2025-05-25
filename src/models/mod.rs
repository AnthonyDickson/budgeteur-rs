//! This module defines the domain data types.

pub use balance::Balance;
pub use category::{Category, CategoryName};
pub use password::{PasswordHash, ValidatedPassword};
pub use transaction::{Transaction, TransactionBuilder};
pub use user::{User, UserID};

mod balance;
mod category;
mod password;
mod transaction;
mod user;

/// Alias for the integer type used for mapping to database IDs.
pub type DatabaseID = i64;
