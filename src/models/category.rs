//! This file defines the `Category` type and the types needed to create a category.
//! A category acts like a tag for a transaction, however a transaction may only have one category.

use std::fmt::Display;

use serde::{Deserialize, Serialize};

use crate::{
    models::{DatabaseID, UserID},
    Error,
};

/// The name of a category.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct CategoryName(String);

impl CategoryName {
    /// Create a category name.
    ///
    /// # Errors
    ///
    /// This function will return an error if `name` is an empty string.
    pub fn new(name: &str) -> Result<Self, Error> {
        if name.is_empty() {
            Err(Error::EmptyCategoryName)
        } else {
            Ok(Self(name.to_string()))
        }
    }

    /// Create a category name without validation.
    ///
    /// The caller should ensure that the string is not empty.
    ///
    /// This function has `_unchecked` in the name but is not `unsafe`, because if the non-empty invariant is violated it will cause incorrect behaviour but not affect memory safety.
    pub fn new_unchecked(name: &str) -> Self {
        Self(name.to_string())
    }
}

impl AsRef<str> for CategoryName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Display for CategoryName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A category for expenses and income, e.g., 'Groceries', 'Eating Out', 'Wages'.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct Category {
    id: DatabaseID,
    name: CategoryName,
    user_id: UserID,
}

impl Category {
    /// Create a new category.
    pub fn new(id: DatabaseID, name: CategoryName, user_id: UserID) -> Self {
        Self { id, name, user_id }
    }

    /// The id of the category.
    pub fn id(&self) -> DatabaseID {
        self.id
    }

    /// The name of the category.
    pub fn name(&self) -> &CategoryName {
        &self.name
    }

    /// The id of the user that created the category.
    pub fn user_id(&self) -> UserID {
        self.user_id
    }
}

#[cfg(test)]
mod category_name_tests {
    use crate::models::category::{CategoryName, Error};

    #[test]
    fn new_fails_on_empty_string() {
        let category_name = CategoryName::new("");

        assert_eq!(category_name, Err(Error::EmptyCategoryName));
    }

    #[test]
    fn new_succeeds_on_non_empty_string() {
        let category_name = CategoryName::new("🔥");

        assert!(category_name.is_ok())
    }
}
