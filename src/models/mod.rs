//! This module defines the domain data types.

pub use category::{Category, CategoryName, NewCategory};
pub use password::{PasswordError, PasswordHash, ValidatedPassword};
pub use recurring_transaction::{Frequency, NewRecurringTransaction, RecurringTransaction};
pub use savings_ratio::{NewSavingsRatio, Ratio, SavingsRatio};
pub use transaction::{NewTransaction, Transaction};
pub use user::{User, UserError, UserID};

mod category;
mod password;
mod recurring_transaction;
mod savings_ratio;
mod transaction;
mod user;
pub type DatabaseID = i64;
