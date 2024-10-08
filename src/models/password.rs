//! This file defines types that handle password validation and hashing.
//! `ValidatedPassword` wraps a string and ensure it is a strong password.
//! `PasswordHash` converts a `ValidatedPassword` into a salted and hashed password.

use std::fmt::Display;

use bcrypt::{hash, verify, BcryptError, DEFAULT_COST};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use zxcvbn::{feedback::Feedback, zxcvbn, Score};

/// Errors that can occur when creating or hashing a password.
#[derive(Debug, Error)]
pub enum PasswordError {
    /// The provided password is too easy to guess.
    #[error("password is too weak: {0}")]
    TooWeak(String),

    /// An unexpected error occurred with the underlying hashing library.
    ///
    /// The error string should only be logged for debugging on the server.
    /// When communicating with the application client this error should be replaced with a general error type indicating an internal server error.
    #[error("hashing failed: {0}")]
    HashingError(String),
}

/// A password that has been validated, but not yet hashed.
///
/// This struct can be used to construct a [PasswordHash].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValidatedPassword(String);

impl ValidatedPassword {
    /// Create and validate a new password from a string.
    ///
    /// # Errors
    ///
    /// This function will return an error if the password is considered too weak.
    /// The error message will explain why the password is considered too weak and suggest how to make it stronger.
    pub fn new(raw_password_string: String) -> Result<Self, PasswordError> {
        let password_analysis = zxcvbn(&raw_password_string, &[]);

        match password_analysis.score() {
            Score::Three | Score::Four => Ok(Self(raw_password_string)),
            _ => Err(PasswordError::TooWeak(
                password_analysis
                    .feedback()
                    .unwrap_or(&Feedback::default())
                    .to_string(),
            )),
        }
    }

    /// Create a new `ValidatedPassword` without any validation.
    ///
    /// The caller should ensure that `raw_password_string` is a valid and secure password.
    ///
    /// This function has `_unchecked` in the name but is not `unsafe`, because if an invalid password is provided it may cause incorrect behaviour but will not affect memory safety.
    pub fn new_unchecked(raw_password_string: String) -> Self {
        Self(raw_password_string)
    }
}

impl Display for ValidatedPassword {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", str::repeat("*", 8))
    }
}

/// A salted and hashed password.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PasswordHash(String);

impl PasswordHash {
    /// Create a hashed password from a validated password.
    ///
    /// # Errors
    ///
    /// This function will return an error if the password could not be hashed.
    pub fn new(password: ValidatedPassword) -> Result<Self, PasswordError> {
        match hash(&password.0, DEFAULT_COST) {
            Ok(password_hash) => Ok(Self(password_hash)),
            Err(e) => Err(PasswordError::HashingError(e.to_string())),
        }
    }

    /// Create a new `PasswordHash` without any validation.
    ///
    /// The caller should ensure that `raw_password_hash` is a valid password hash.
    ///
    /// This function has `_unchecked` in the name but is not `unsafe`, because if an invalid hash is provided it will cause incorrect behaviour but not affect memory safety.
    pub fn new_unchecked(raw_password_hash: String) -> Self {
        Self(raw_password_hash)
    }

    /// Try to create a password hash from a raw password string.
    ///
    /// This is a convenience function that allows one to skip the intermediate `ValidatedPassword`
    /// type.
    ///
    /// This function is used instead of `From<String>` or `FromStr` to make it a bit clearer that
    /// we are not parsing an existing password hash.
    pub fn from_string(raw_password: String) -> Result<Self, PasswordError> {
        let validated_password = ValidatedPassword::new(raw_password)?;
        PasswordHash::new(validated_password)
    }

    /// Check that `raw_password` matches the stored password.
    pub fn verify(&self, raw_password: &str) -> Result<bool, BcryptError> {
        verify(raw_password, &self.0)
    }
}

impl Display for PasswordHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod validated_password_tests {
    use crate::models::{PasswordError, ValidatedPassword};

    #[test]
    fn new_fails_on_empty() {
        let result = ValidatedPassword::new("".to_string());

        assert!(matches!(result, Err(PasswordError::TooWeak(_))));
    }

    #[test]
    fn new_fails_on_short_password() {
        let result = ValidatedPassword::new("imtooshort".to_string());

        assert!(matches!(result, Err(PasswordError::TooWeak(_))));
    }

    #[test]
    fn new_succeeds_on_long_password() {
        let result = ValidatedPassword::new("asomewhatlongpassword1".to_string());

        assert!(result.is_ok());
    }
}

#[cfg(test)]
mod password_hash_tests {
    use crate::models::{PasswordHash, ValidatedPassword};

    #[test]
    fn verify_password_succeeds_for_valid_password() {
        let hash = PasswordHash::new_unchecked(
            "$2b$12$Gwf0uvxH3L7JLfo0CC/NCOoijK2vQ/wbgP.LeNup8vj6gg31IiFkm".to_owned(),
        );
        let password = "okon";

        assert!(hash.verify(password).unwrap());
    }

    #[test]
    fn verify_password_fails_for_invalid_password() {
        let hash = PasswordHash::new_unchecked(
            "$2b$12$Gwf0uvxH3L7JLfo0CC/NCOoijK2vQ/wbgP.LeNup8vj6gg31IiFkm".to_owned(),
        );
        let password = "thewrongpassword";

        assert!(!hash.verify(password).unwrap());
    }

    #[test]
    fn hash_password_produces_verifiable_hash() {
        let password = "roostersgocockledoodledoo";
        let wrong_password = "the_wrong_password";
        let hash = PasswordHash::new(ValidatedPassword::new(password.to_owned()).unwrap()).unwrap();

        assert!(hash.verify(password).unwrap());
        assert!(!hash.verify(wrong_password).unwrap());
    }

    #[test]
    fn hash_duplicate_password_produces_unique_hash() {
        let password = ValidatedPassword::new("turkeysgogobblegobble".to_owned()).unwrap();
        let hash = PasswordHash::new(password.clone()).unwrap();
        let dupe_hash = PasswordHash::new(password.clone()).unwrap();

        assert_ne!(hash, dupe_hash);
    }

    #[test]
    fn from_string_fails_on_weak_password() {
        let hash = PasswordHash::from_string("password1234".to_owned());

        assert!(hash.is_err());
    }

    #[test]
    fn from_string_succeeds_on_string_password() {
        let hash = PasswordHash::from_string("thisisaverysecurepassword!!!!".to_owned());

        assert!(hash.is_ok());
    }
}
