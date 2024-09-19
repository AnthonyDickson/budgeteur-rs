//! This module defines the domain data types.

pub use category::{Category, CategoryBuilder, CategoryError, CategoryName};
pub use password::{PasswordError, PasswordHash, ValidatedPassword};
pub use transaction::{NewTransaction, Transaction};
pub use user::{User, UserBuilder, UserError, UserID};

mod category;
mod password;
mod transaction;
mod user;
pub type DatabaseID = i64;
