//! Defines the category store trait.

use crate::{
    Error,
    models::{Category, CategoryName, DatabaseID, UserID},
};

/// Creates and retrieves transaction categories for transactions.
pub trait CategoryStore {
    /// Create a new category and add it the store.
    fn create(&self, name: CategoryName, user_id: UserID) -> Result<Category, Error>;

    /// Get a category by its ID.
    fn get(&self, category_id: DatabaseID) -> Result<Category, Error>;

    /// Get all categories for a given user.
    fn get_by_user(&self, user_id: UserID) -> Result<Vec<Category>, Error>;
}
