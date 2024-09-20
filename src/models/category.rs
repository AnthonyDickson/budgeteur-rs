//! This file defines the `Category` type and the types needed to create a category.
//! A category acts like a tag for a transaction, however a transaction may only have one category.

use rusqlite::{Connection, Row};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    db::{CreateTable, MapRow},
    models::{DatabaseID, UserID},
};

#[derive(Debug, Error, PartialEq)]
pub enum CategoryError {
    #[error("a category with the given details could not found in the database")]
    NotFound,

    #[error("an empty string is not a valid category name")]
    InvalidName,

    #[error("the user ID does not refer to a valid user.")]
    InvalidUser,

    #[error("an unexpected error occurred: {0}")]
    SqlError(rusqlite::Error),
}

/// The name of a category.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct CategoryName(String);

impl CategoryName {
    /// Create a category name.
    ///
    /// # Errors
    ///
    /// This function will return an error if `name` is an empty string.
    pub fn new(name: String) -> Result<Self, CategoryError> {
        if name.is_empty() {
            Err(CategoryError::InvalidName)
        } else {
            Ok(Self(name))
        }
    }

    /// Create a category name without validation.
    ///
    /// The caller should ensure that the string is not empty.
    ///
    /// This function has `_unchecked` in the name but is not `unsafe`, because if the non-empty invariant is violated it will cause incorrect behaviour but not affect memory safety.
    pub fn new_unchecked(name: String) -> Self {
        Self(name)
    }
}

impl AsRef<str> for CategoryName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<rusqlite::Error> for CategoryError {
    fn from(value: rusqlite::Error) -> Self {
        match value {
            // Code 787 occurs when a FOREIGN KEY constraint failed.
            rusqlite::Error::SqliteFailure(error, Some(_)) if error.extended_code == 787 => {
                CategoryError::InvalidUser
            }
            rusqlite::Error::QueryReturnedNoRows => CategoryError::NotFound,
            error => CategoryError::SqlError(error),
        }
    }
}

/// A category for expenses and income, e.g., 'Groceries', 'Eating Out', 'Wages'.
///
/// To create a new category, use [Category::build]. To retrieve an existing category from the
/// application database, use [Category::select] to get a category by its ID and
/// [Category::select_by_user] to get a user's categories.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct Category {
    id: DatabaseID,
    name: CategoryName,
    user_id: UserID,
}

impl Category {
    /// Create a new category.
    ///
    /// Shortcut for [CategoryBuilder] for discoverability.
    ///
    /// If you are trying to get an existing category, use [Category::select] or [Category::select_by_user].
    pub fn build(name: CategoryName, user_id: UserID) -> CategoryBuilder {
        CategoryBuilder::new(name, user_id)
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

impl Category {
    /// Retrieve categories in the database for the user `user_id`.
    ///
    /// # Errors
    /// This function will return an error if there is an SQL error.
    pub fn select(category_id: DatabaseID, connection: &Connection) -> Result<Self, CategoryError> {
        connection
            .prepare("SELECT id, name, user_id FROM category WHERE id = :id")?
            .query_row(&[(":id", &category_id)], Category::map_row)
            .map_err(|error| error.into())
    }

    /// Retrieve categories in the database for the user `user_id`.
    ///
    /// # Errors
    /// This function will return an error if there is an SQL error.
    pub fn select_by_user(
        user_id: UserID,
        connection: &Connection,
    ) -> Result<Vec<Self>, CategoryError> {
        connection
            .prepare("SELECT id, name, user_id FROM category WHERE user_id = :user_id")?
            .query_map(&[(":user_id", &user_id.as_i64())], Category::map_row)?
            .map(|maybe_category| maybe_category.map_err(CategoryError::SqlError))
            .collect()
    }
}

impl CreateTable for Category {
    fn create_table(connection: &Connection) -> Result<(), rusqlite::Error> {
        connection.execute(
            "CREATE TABLE category (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                user_id INTEGER NOT NULL,
                FOREIGN KEY(user_id) REFERENCES user(id) ON UPDATE CASCADE ON DELETE CASCADE,
                UNIQUE(user_id, name) ON CONFLICT ROLLBACK
                )",
            (),
        )?;

        Ok(())
    }
}

impl MapRow for Category {
    type ReturnType = Self;

    fn map_row_with_offset(row: &Row, offset: usize) -> Result<Self, rusqlite::Error> {
        let id = row.get(offset)?;

        let raw_name: String = row.get(offset + 1)?;
        let name = CategoryName::new_unchecked(raw_name);

        let raw_user_id = row.get(offset + 2)?;
        let user_id = UserID::new(raw_user_id);

        Ok(Self { id, name, user_id })
    }
}

/// Builder for creating a new [Category].
///
/// The function for finalizing the builder is [CategoryBuilder::insert].
///
/// If you are trying to retrieve an existing category, see [Category::select] and
/// [Category::select_by_user].
pub struct CategoryBuilder {
    name: CategoryName,
    user_id: UserID,
}

impl CategoryBuilder {
    /// Create a new category and insert it into the application database.
    ///
    /// Finalize the builder with [CategoryBuilder::insert].
    ///
    /// If you are trying to retrieve an existing category, see [Category::select] and
    /// [Category::select_by_user].
    fn new(name: CategoryName, user_id: UserID) -> Self {
        Self { name, user_id }
    }

    /// Insert the category into the application database and return the built category.
    /// Note that this function will consume the builder.
    ///
    /// # Errors
    ///
    /// This function will return a:
    /// - [CategoryError::InvalidUser] if the given user ID does not refer to a valid user.
    /// - [CategoryError::SqlError] if there was an unexpected SQL error.
    pub fn insert(self, connection: &Connection) -> Result<Category, CategoryError> {
        connection.execute(
            "INSERT INTO category (name, user_id) VALUES (?1, ?2)",
            (self.name.as_ref(), self.user_id.as_i64()),
        )?;

        let id = connection.last_insert_rowid();

        Ok(Category {
            id,
            name: self.name,
            user_id: self.user_id,
        })
    }
}

#[cfg(test)]
mod category_name_tests {
    use crate::models::category::{CategoryError, CategoryName};

    #[test]
    fn new_fails_on_empty_string() {
        let category_name = CategoryName::new("".to_string());

        assert_eq!(category_name, Err(CategoryError::InvalidName));
    }

    #[test]
    fn new_succeeds_on_non_empty_string() {
        let category_name = CategoryName::new("🔥".to_string());

        assert!(category_name.is_ok())
    }
}

#[cfg(test)]
mod category_tests {
    use std::{collections::HashSet, str::FromStr};

    use email_address::EmailAddress;
    use rusqlite::Connection;

    use crate::{
        db::initialize,
        models::{category::CategoryError, Category, CategoryName, PasswordHash, User, UserID},
    };

    fn init_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();
        conn
    }

    fn create_database_and_insert_test_user() -> (Connection, User) {
        let conn = init_db();

        let test_user = User::build(
            EmailAddress::from_str("foo@bar.baz").unwrap(),
            PasswordHash::new_unchecked("hunter2".to_string()),
        )
        .insert(&conn)
        .unwrap();

        (conn, test_user)
    }

    #[test]
    fn insert_category_succeeds() {
        let (conn, test_user) = create_database_and_insert_test_user();

        let name = CategoryName::new("Categorically a category".to_string()).unwrap();

        let category = Category::build(name.clone(), test_user.id())
            .insert(&conn)
            .unwrap();

        assert!(category.id() > 0);
        assert_eq!(category.name(), &name);
        assert_eq!(category.user_id(), test_user.id());
    }

    #[test]
    fn insert_category_fails_with_invalid_user_id() {
        let conn = init_db();
        let name = CategoryName::new("Foo".to_string()).unwrap();

        let maybe_category = Category::build(name, UserID::new(42)).insert(&conn);

        assert_eq!(maybe_category, Err(CategoryError::InvalidUser));
    }

    #[test]
    fn select_category_succeeds() {
        let (conn, test_user) = create_database_and_insert_test_user();
        let name = CategoryName::new_unchecked("Foo".to_string());
        let inserted_category = Category::build(name, test_user.id()).insert(&conn).unwrap();

        let selected_category = Category::select(inserted_category.id(), &conn).unwrap();

        assert_eq!(inserted_category, selected_category);
    }

    #[test]
    fn select_category_fails_with_invalid_id() {
        let conn = init_db();

        let selected_category = Category::select(1337, &conn);

        assert_eq!(selected_category, Err(CategoryError::NotFound));
    }

    #[test]
    fn select_category_with_user_id() {
        let (conn, test_user) = create_database_and_insert_test_user();
        let inserted_categories = HashSet::from([
            Category::build(
                CategoryName::new_unchecked("Foo".to_string()),
                test_user.id(),
            )
            .insert(&conn)
            .unwrap(),
            Category::build(
                CategoryName::new_unchecked("Bar".to_string()),
                test_user.id(),
            )
            .insert(&conn)
            .unwrap(),
        ]);

        let selected_categories = Category::select_by_user(test_user.id(), &conn).unwrap();
        let selected_categories = HashSet::from_iter(selected_categories);

        assert_eq!(inserted_categories, selected_categories);
    }

    #[test]
    fn select_category_with_invalid_user_id() {
        let (conn, test_user) = create_database_and_insert_test_user();

        Category::build(
            CategoryName::new_unchecked("Foo".to_string()),
            test_user.id(),
        )
        .insert(&conn)
        .unwrap();

        Category::build(
            CategoryName::new_unchecked("Bar".to_string()),
            test_user.id(),
        )
        .insert(&conn)
        .unwrap();

        let selected_categories =
            Category::select_by_user(UserID::new(test_user.id().as_i64() + 1), &conn).unwrap();

        assert_eq!(selected_categories, []);
    }
}
