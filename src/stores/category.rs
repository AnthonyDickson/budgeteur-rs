//! Defines the category store trait.

use crate::{
    Error,
    models::{Category, CategoryName, DatabaseID},
};

/// Creates and retrieves transaction categories for transactions.
pub trait CategoryStore {
    /// Create a new category and add it the store.
    fn create(&self, name: CategoryName) -> Result<Category, Error>;

    /// Get a category by its ID.
    fn get(&self, category_id: DatabaseID) -> Result<Category, Error>;

    /// Get all categories.
    fn get_all(&self) -> Result<Vec<Category>, Error>;
}
