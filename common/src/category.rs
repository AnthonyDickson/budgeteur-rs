use serde::{Deserialize, Serialize};

use crate::{DatabaseID, UserID};

#[derive(thiserror::Error, Debug)]
#[error("{0} is not a valid category name")]
pub struct CategoryNameError(pub String);

/// The name of a category.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CategoryName(String);

impl CategoryName {
    /// Create a category name.
    ///
    /// # Errors
    ///
    /// This function will return an error if `name` is an empty string.
    pub fn new(name: String) -> Result<Self, CategoryNameError> {
        if name.is_empty() {
            Err(CategoryNameError(name))
        } else {
            Ok(Self(name))
        }
    }

    /// Create a category name without validation.
    ///
    /// # Safety
    ///
    /// This function should only be called on strings coming out of a trusted source such as the application's database.
    pub unsafe fn new_unchecked(name: String) -> Self {
        Self(name)
    }
}

impl AsRef<str> for CategoryName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// A category for expenses and income, e.g., 'Groceries', 'Eating Out', 'Wages'.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
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
    use crate::{category::CategoryName, CategoryNameError};

    #[test]
    fn new_fails_on_empty_string() {
        let category_name = CategoryName::new("".to_string());

        assert!(matches!(category_name, Err(CategoryNameError(_))))
    }

    #[test]
    fn new_succeeds_on_non_empty_string() {
        let category_name = CategoryName::new("🔥".to_string());

        assert!(category_name.is_ok())
    }
}
