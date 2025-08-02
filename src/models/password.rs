//! This file defines types that handle password validation and hashing.
//! `ValidatedPassword` wraps a string and ensure it is a strong password.
//! `PasswordHash` converts a `ValidatedPassword` into a salted and hashed password.

use std::fmt::Display;

use bcrypt::{BcryptError, hash, verify};
use serde::{Deserialize, Serialize};
use zxcvbn::{Score, feedback::Feedback, zxcvbn};

use crate::Error;

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
    pub fn new(raw_password_string: &str) -> Result<Self, Error> {
        let password_analysis = zxcvbn(raw_password_string, &[]);

        match password_analysis.score() {
            Score::Three | Score::Four => Ok(Self(raw_password_string.to_string())),
            _ => Err(Error::TooWeak(
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
    pub fn new_unchecked(raw_password_string: &str) -> Self {
        Self(raw_password_string.to_string())
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
    /// An alias for the default encryption cost for hashing passwords.
    pub const DEFAULT_COST: u32 = bcrypt::DEFAULT_COST;

    /// Create a hashed password from a validated password with the specified `cost`.
    ///
    /// `cost` increases the rounds of hashing and therefore the time needed to verify a password.
    /// A value of at least 12 is recommended. Pass in [PasswordHash::DEFAULT_COST] to use the recommended cost.
    ///
    /// # Errors
    ///
    /// This function will return an error if the password could not be hashed.
    pub fn new(password: ValidatedPassword, cost: u32) -> Result<Self, Error> {
        match hash(&password.0, cost) {
            Ok(password_hash) => Ok(Self(password_hash)),
            Err(e) => Err(Error::HashingError(e.to_string())),
        }
    }

    /// Create a new `PasswordHash` without any validation.
    ///
    /// The caller should ensure that `raw_password_hash` is a valid password hash.
    ///
    /// This function has `_unchecked` in the name but is not `unsafe`, because if an invalid hash is provided it will cause incorrect behaviour but not affect memory safety.
    pub fn new_unchecked(raw_password_hash: &str) -> Self {
        Self(raw_password_hash.to_string())
    }

    /// Try to create a password hash from a raw password string.
    ///
    /// This is a convenience function that removes the need to manually create
    /// the intermediate `ValidatedPassword` type.
    ///
    /// This function is used instead of `From<String>` or `FromStr` to make it a bit clearer that
    /// we are not parsing an existing password hash.
    pub fn from_raw_password(raw_password: &str, cost: u32) -> Result<Self, Error> {
        let validated_password = ValidatedPassword::new(raw_password)?;
        PasswordHash::new(validated_password, cost)
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
    use crate::{Error, models::ValidatedPassword};

    #[test]
    fn new_fails_on_empty() {
        let result = ValidatedPassword::new("");

        assert!(matches!(result, Err(Error::TooWeak(_))));
    }

    #[test]
    fn new_fails_on_short_password() {
        let result = ValidatedPassword::new("imtooshort");

        assert!(matches!(result, Err(Error::TooWeak(_))));
    }

    #[test]
    fn new_succeeds_on_long_password() {
        let result = ValidatedPassword::new("asomewhatlongpassword1");

        assert!(result.is_ok());
    }
}

#[cfg(test)]
mod password_hash_tests {
    use crate::models::{PasswordHash, ValidatedPassword};

    #[test]
    fn verify_password_succeeds_for_valid_password() {
        let hash = PasswordHash::new_unchecked(
            "$2b$12$Gwf0uvxH3L7JLfo0CC/NCOoijK2vQ/wbgP.LeNup8vj6gg31IiFkm",
        );
        let password = "okon";

        assert!(hash.verify(password).unwrap());
    }

    #[test]
    fn verify_password_fails_for_invalid_password() {
        let hash = PasswordHash::new_unchecked(
            "$2b$12$Gwf0uvxH3L7JLfo0CC/NCOoijK2vQ/wbgP.LeNup8vj6gg31IiFkm",
        );
        let password = "thewrongpassword";

        assert!(!hash.verify(password).unwrap());
    }

    #[test]
    fn hash_password_produces_verifiable_hash() {
        let password = "roostersgocockledoodledoo";
        let wrong_password = "the_wrong_password";
        let hash = PasswordHash::from_raw_password(password, 4).unwrap();

        assert!(hash.verify(password).unwrap());
        assert!(!hash.verify(wrong_password).unwrap());
    }

    #[test]
    fn hash_duplicate_password_produces_unique_hash() {
        let password = ValidatedPassword::new("turkeysgogobblegobble").unwrap();
        let hash = PasswordHash::new(password.clone(), 4).unwrap();
        let dupe_hash = PasswordHash::new(password.clone(), 4).unwrap();

        assert_ne!(hash, dupe_hash);
    }

    #[test]
    fn from_string_fails_on_weak_password() {
        let hash = PasswordHash::from_raw_password("password1234", 4);

        assert!(hash.is_err());
    }

    #[test]
    fn from_string_succeeds_on_string_password() {
        let hash = PasswordHash::from_raw_password("thisisaverysecurepassword!!!!", 4);

        assert!(hash.is_ok());
    }
}
