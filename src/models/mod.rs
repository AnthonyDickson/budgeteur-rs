//! This module defines the domain data types.

pub use password::{PasswordHash, ValidatedPassword};

mod password;

/// Alias for the integer type used for mapping to database IDs.
pub type DatabaseID = i64;
