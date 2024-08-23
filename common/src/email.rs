use serde::{Deserialize, Serialize};
use std::{fmt::Display, ops::Deref};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Email(String);

#[derive(Error, Debug, Clone, PartialEq)]
#[error("{0} is not a valid email address")]
pub struct EmailAddressError(pub String);

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
