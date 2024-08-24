use std::{fmt::Display, ops::Deref};

use bcrypt::{hash, verify, BcryptError, DEFAULT_COST};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
#[error("{0} is not a valid password")]
pub struct PasswordError(pub String);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PasswordHash(String);

impl PasswordHash {
    /// Create a hashed password from a validated password.
    ///
    /// # Errors
    ///
    /// This function will return an error if the password could not be hashed.
    pub fn new(raw_password: RawPassword) -> Result<Self, PasswordError> {
        match hash(&raw_password, DEFAULT_COST) {
            Ok(password_hash) => Ok(Self(password_hash)),
            // TODO: Add error variants to `PasswordError` to indicate error w/ hashing.
            Err(_) => Err(PasswordError(raw_password.into_string())),
        }
    }

    /// Create a new `PasswordHash` without any validation.
    ///
    /// This is intended to be used with a valid password hash.
    ///
    /// # Safety
    ///
    /// This function should only be called on strings coming out of a trusted source such as the application's database.
    pub unsafe fn new_unchecked(raw_password_hash: String) -> Self {
        Self(raw_password_hash)
    }

    /// Check that `raw_password` matches the stored password.
    pub fn verify(&self, raw_password: &RawPassword) -> Result<bool, BcryptError> {
        verify(raw_password, self)
    }
}

impl Display for PasswordHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsRef<str> for PasswordHash {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl AsRef<[u8]> for PasswordHash {
    fn as_ref(&self) -> &[u8] {
        self.0.as_bytes()
    }
}

impl Deref for PasswordHash {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[cfg(test)]
mod password_hash_tests {
    use crate::{PasswordHash, RawPassword};

    #[test]
    fn verify_password_succeeds_for_valid_password() {
        let hash = unsafe {
            PasswordHash::new_unchecked(
                "$2b$12$Gwf0uvxH3L7JLfo0CC/NCOoijK2vQ/wbgP.LeNup8vj6gg31IiFkm".to_owned(),
            )
        };
        let password = unsafe { RawPassword::new_unchecked("okon".to_owned()) };

        assert!(hash.verify(&password).unwrap());
    }

    #[test]
    fn verify_password_fails_for_invalid_password() {
        let hash = unsafe {
            PasswordHash::new_unchecked(
                "$2b$12$Gwf0uvxH3L7JLfo0CC/NCOoijK2vQ/wbgP.LeNup8vj6gg31IiFkm".to_owned(),
            )
        };
        let password = unsafe { RawPassword::new_unchecked("thewrongpassword".to_owned()) };

        assert!(!hash.verify(&password).unwrap());
    }

    #[test]
    fn hash_password_produces_verifiable_hash() {
        let password = RawPassword::new("password123456".to_owned()).unwrap();
        let wrong_password = RawPassword::new("the_wrong_password".to_owned()).unwrap();
        let hash = PasswordHash::new(password.clone()).unwrap();

        assert!(hash.verify(&password).unwrap());
        assert!(!hash.verify(&wrong_password).unwrap());
    }

    #[test]
    fn hash_duplicate_password_produces_unique_hash() {
        let password = RawPassword::new("password123456".to_owned()).unwrap();
        let hash = PasswordHash::new(password.clone()).unwrap();
        let dupe_hash = PasswordHash::new(password.clone()).unwrap();

        assert_ne!(hash, dupe_hash);
    }
}

/// A password that has been validated, but not yet hashed.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RawPassword(String);

impl RawPassword {
    /// Create a new password from a string.
    ///
    /// # Errors
    ///
    /// This function will return an error if the password is less than 14 characters long.
    pub fn new(raw_password_string: String) -> Result<Self, PasswordError> {
        // TODO: More thorough validation of passwords.
        if raw_password_string.chars().count() < 14 {
            Err(PasswordError(raw_password_string))
        } else {
            Ok(Self(raw_password_string))
        }
    }

    /// Create a new `RawPassword` without any validation.
    ///
    /// # Safety
    ///
    /// This function should only be called on strings coming out of a trusted source such as the application's database or for tests where costly validation is unecessary.
    pub unsafe fn new_unchecked(raw_password_string: String) -> Self {
        Self(raw_password_string)
    }
}

impl AsRef<str> for RawPassword {
    fn as_ref(&self) -> &str {
        &self.0
    }
}
impl AsRef<[u8]> for RawPassword {
    fn as_ref(&self) -> &[u8] {
        self.0.as_bytes()
    }
}

impl RawPassword {
    fn into_string(self) -> String {
        self.0
    }
}

#[cfg(test)]
mod raw_password_tests {
    use crate::{PasswordError, RawPassword};

    #[test]
    fn new_fails_on_empty() {
        let result = RawPassword::new("".to_string());

        assert!(matches!(result, Err(PasswordError(_))));
    }

    #[test]
    fn new_fails_on_short_password() {
        let result = RawPassword::new("imtooshort".to_string());

        assert!(matches!(result, Err(PasswordError(_))));
    }

    #[test]
    fn new_succeeds_on_long_password() {
        let result = RawPassword::new("alongpassword1".to_string());

        assert!(result.is_ok());
    }
}
