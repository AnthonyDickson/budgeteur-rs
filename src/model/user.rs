use email_address::EmailAddress;
use serde::{Deserialize, Serialize};

use crate::model::PasswordHash;

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
