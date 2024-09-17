//! This file defines a user of the application and its supporting types.

use std::fmt::Display;

use email_address::EmailAddress;
use serde::{Deserialize, Serialize};

use crate::models::PasswordHash;

/// A newtype wrapper for integer user IDs.
/// This helps disambiguate user IDs from other types of IDs, leading to better compile time
/// errors, and more flexible generics that can have distinct implementations for multiple ID types.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct UserID(i64);

impl UserID {
    pub fn new(id: i64) -> Self {
        Self(id)
    }

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
    pub fn new(id: UserID, email: EmailAddress, password_hash: PasswordHash) -> Self {
        User {
            id,
            email,
            password_hash,
        }
    }

    pub fn id(&self) -> UserID {
        self.id
    }

    pub fn email(&self) -> &EmailAddress {
        &self.email
    }

    pub fn password_hash(&self) -> &PasswordHash {
        &self.password_hash
    }
}

/// The data for creating a new user.
pub struct NewUser {
    pub email: EmailAddress,
    pub password_hash: PasswordHash,
}

// `User` is a simple container with no variants (the contained types handle this), so there isn't much to test.
