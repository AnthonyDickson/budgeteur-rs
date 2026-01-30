//! Core tag domain types.

use std::{fmt::Display, str::FromStr};

use serde::{Deserialize, Serialize};

use crate::Error;

/// A validated, non-empty tag name.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct TagName(String);

impl TagName {
    /// Create a tag name.
    ///
    /// # Errors
    ///
    /// This function will return an [Error::EmptyTagName] if `name` is an empty string.
    pub fn new(name: &str) -> Result<Self, Error> {
        let name = name.trim();

        if name.is_empty() {
            Err(Error::EmptyTagName)
        } else {
            Ok(Self(name.to_string()))
        }
    }

    /// Create a tag name without validation.
    ///
    /// The caller should ensure that the string is not empty.
    ///
    /// This function has `_unchecked` in the name but is not `unsafe`, because if the non-empty invariant is violated it will cause incorrect behaviour but not affect memory safety.
    pub fn new_unchecked(name: &str) -> Self {
        Self(name.to_string())
    }
}

impl AsRef<str> for TagName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl FromStr for TagName {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        TagName::new(s)
    }
}

impl Display for TagName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Database identifier for a tag.
pub type TagId = i64;

/// A tag for categorizing transactions (e.g., 'Groceries', 'Salary').
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct Tag {
    pub id: TagId,
    pub name: TagName,
}

/// Form data for tag creation and editing.
#[derive(Debug, Serialize, Deserialize)]
pub struct TagFormData {
    pub name: String,
}
