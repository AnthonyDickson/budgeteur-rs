//! This module defines the domain data types.

pub use category::{Category, CategoryName, NewCategory};
pub use password::{PasswordError, PasswordHash, ValidatedPassword};
pub use transaction::{NewTransaction, Transaction};
pub use user::{User, UserError, UserID};

mod category;
mod password;
mod transaction;
mod user;
pub type DatabaseID = i64;
