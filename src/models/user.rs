//! This file defines a user of the application and its supporting types.

use std::fmt::Display;

use email_address::EmailAddress;
use serde::{Deserialize, Serialize};

use crate::models::PasswordHash;

/// A newtype wrapper for integer user IDs.
///
/// This helps disambiguate user IDs from other types of IDs, leading to better compile time
/// errors, and more flexible generics that can have distinct implementations for multiple ID types.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct UserID(i64);

impl UserID {
    /// Create a new user ID.
    pub fn new(id: i64) -> Self {
        Self(id)
    }

    /// Cast the user ID to a 64 bit integer.
    pub fn as_i64(&self) -> i64 {
        self.0
    }
}

impl Display for UserID {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// A user of the application.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct User {
    id: UserID,
    email: EmailAddress,
    password_hash: PasswordHash,
}

impl User {
    /// Create a new user.
    ///
    /// The caller should ensure that `id` is unique.
    pub fn new(id: UserID, email: EmailAddress, password_hash: PasswordHash) -> Self {
        Self {
            id,
            email,
            password_hash,
        }
    }

    /// The user's ID in the database.
    pub fn id(&self) -> UserID {
        self.id
    }

    /// The email address associated with the user.
    pub fn email(&self) -> &EmailAddress {
        &self.email
    }

    /// The user's password hash.
    pub fn password_hash(&self) -> &PasswordHash {
        &self.password_hash
    }
}
