use std::{fmt::Display, ops::Deref};

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub type DatabaseID = i64;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Email(String);

#[derive(Error, Debug, Clone, PartialEq)]
#[error("{0} is not a valid email address")]
pub struct EmailAddressError(String);

impl Email {
    pub fn new(raw_email: &str) -> Result<Self, EmailAddressError> {
        // TODO: Use proper regex/email validation.
        if raw_email.contains('@') && !raw_email.is_empty() {
            Ok(Self(raw_email.to_string()))
        } else {
            Err(EmailAddressError(raw_email.to_string()))
        }
    }
}

impl Display for Email {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsRef<str> for Email {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Deref for Email {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
// TODO: Implement password and password hash newtypes that wrap a string + type state to ensure that raw passwords are not accidentally stored as a password hash (RawPassword should have a function that hashes and salts the password and returns a PasswordHash).

/// A user of the application.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct User {
    id: DatabaseID,
    email: Email,
    password_hash: String,
}

impl User {
    pub fn new(id: DatabaseID, email: Email, password_hash: String) -> Self {
        User {
            id,
            email,
            password_hash,
        }
    }

    pub fn id(&self) -> DatabaseID {
        self.id
    }

    pub fn email(&self) -> &Email {
        &self.email
    }

    pub fn password_hash(&self) -> &str {
        &self.password_hash
    }
}

#[cfg(test)]
mod email_tests {
    use crate::{Email, EmailAddressError};

    #[test]
    fn create_email_success() {
        let email = Email::new("foo@bar.baz");

        assert!(email.is_ok())
    }

    #[test]
    fn create_email_fails_with_no_at_symbol() {
        let email = Email::new("foobar.baz");

        assert!(matches!(email, Err(EmailAddressError(_))));
    }

    #[test]
    fn create_email_fails_with_empty_string() {
        let email = Email::new("");

        assert!(matches!(email, Err(EmailAddressError(_))));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_transaction() {
        let id = 1;
        let email = Email::new("foo@bar.baz").unwrap();
        let password_hash = "definitelyapasswordhash".to_string();

        let user = User::new(id, email.clone(), password_hash.clone());

        assert_eq!(user.id, id);
        assert_eq!(user.email, email);
        assert_eq!(user.password_hash, password_hash);
    }
}
