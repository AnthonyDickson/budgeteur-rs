use serde::{Deserialize, Serialize};
use std::{fmt::Display, ops::Deref};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Email(String);

#[derive(Error, Debug, Clone, PartialEq)]
#[error("{0} is not a valid email address")]
pub struct EmailAddressError(pub String);

impl Email {
    /// Create an email address.
    ///
    /// # Errors
    ///
    /// This function will return an error if `raw_email` is not a valid email address.
    pub fn new(raw_email: &str) -> Result<Self, EmailAddressError> {
        // TODO: Use proper regex/email validation.
        if raw_email.contains('@') && !raw_email.is_empty() {
            Ok(Self(raw_email.to_string()))
        } else {
            Err(EmailAddressError(raw_email.to_string()))
        }
    }

    /// Create a new `Email` without any validation.
    ///
    /// # Safety
    ///
    /// This function should only be called on strings coming out of a trusted source such as the application's database.
    /// For emails coming from the user (e.g., via the REST API), this function should **not** be used, instead use the checked version of this method.
    pub unsafe fn new_unchecked(raw_email: String) -> Self {
        Self(raw_email)
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
