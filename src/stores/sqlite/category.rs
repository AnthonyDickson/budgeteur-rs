//! Implements a SQLite backed category store.

use std::sync::{Arc, Mutex};

use rusqlite::{Connection, Row};

use crate::{
    Error,
    db::{CreateTable, MapRow},
    models::{Category, CategoryName, DatabaseID},
    stores::CategoryStore,
};

/// Creates and retrieves transaction categories to/from a SQLite database.
#[derive(Debug, Clone)]
pub struct SQLiteCategoryStore {
    connection: Arc<Mutex<Connection>>,
}

impl SQLiteCategoryStore {
    /// Create a new category store with a SQLite database.
    pub fn new(connection: Arc<Mutex<Connection>>) -> Self {
        Self { connection }
    }
}

impl CategoryStore for SQLiteCategoryStore {
    /// Create a category in the database.
    ///
    /// # Errors
    /// This function will return an error if there is an SQL error.
    fn create(&self, name: CategoryName) -> Result<Category, Error> {
        let connection = self.connection.lock().unwrap();
        connection.execute("INSERT INTO category (name) VALUES (?1);", (name.as_ref(),))?;

        let id = connection.last_insert_rowid();

        Ok(Category { id, name })
    }

    /// Retrieve categories in the database for the category with `category_id`.
    ///
    /// # Errors
    /// This function will return an error if there is an SQL error.
    fn get(&self, category_id: DatabaseID) -> Result<Category, Error> {
        self.connection
            .lock()
            .unwrap()
            .prepare("SELECT id, name FROM category WHERE id = :id;")?
            .query_row(&[(":id", &category_id)], SQLiteCategoryStore::map_row)
            .map_err(|error| error.into())
    }

    /// Retrieve categories in the database.
    ///
    /// # Errors
    /// This function will return an error if there is an SQL error.
    fn get_all(&self) -> Result<Vec<Category>, Error> {
        self.connection
            .lock()
            .unwrap()
            .prepare("SELECT id, name FROM category;")?
            .query_map([], SQLiteCategoryStore::map_row)?
            .map(|maybe_category| maybe_category.map_err(|error| error.into()))
            .collect()
    }
}

impl CreateTable for SQLiteCategoryStore {
    fn create_table(connection: &Connection) -> Result<(), rusqlite::Error> {
        connection.execute(
            "CREATE TABLE IF NOT EXISTS category (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL UNIQUE
            );",
            (),
        )?;

        Ok(())
    }
}

impl MapRow for SQLiteCategoryStore {
    type ReturnType = Category;

    fn map_row_with_offset(row: &Row, offset: usize) -> Result<Self::ReturnType, rusqlite::Error> {
        let id = row.get(offset)?;

        let raw_name: String = row.get(offset + 1)?;
        let name = CategoryName::new_unchecked(&raw_name);

        Ok(Self::ReturnType { id, name })
    }
}

#[cfg(test)]
mod category_tests {
    use std::collections::HashSet;
    use std::sync::{Arc, Mutex};

    use rusqlite::Connection;

    use crate::{Error, db::initialize, models::CategoryName};

    use super::{CategoryStore, SQLiteCategoryStore};

    fn get_test_store() -> SQLiteCategoryStore {
        let connection = Connection::open_in_memory().unwrap();
        initialize(&connection).unwrap();
        let connection = Arc::new(Mutex::new(connection));

        SQLiteCategoryStore::new(connection.clone())
    }

    #[test]
    fn create_category_succeeds() {
        let store = get_test_store();
        let name = CategoryName::new("Categorically a category").unwrap();

        let category = store.create(name.clone()).unwrap();

        assert!(category.id > 0);
        assert_eq!(category.name, name);
    }

    #[test]
    fn get_category_succeeds() {
        let store = get_test_store();
        let name = CategoryName::new_unchecked("Foo");
        let inserted_category = store.create(name).unwrap();

        let selected_category = store.get(inserted_category.id);

        assert_eq!(Ok(inserted_category), selected_category);
    }

    #[test]
    fn get_category_with_invalid_id_returns_not_found() {
        let store = get_test_store();
        let inserted_category = store.create(CategoryName::new_unchecked("Foo")).unwrap();

        let selected_category = store.get(inserted_category.id + 123);

        assert_eq!(selected_category, Err(Error::NotFound));
    }

    #[test]
    fn get_all_categories() {
        let store = get_test_store();

        let inserted_categories = HashSet::from([
            store.create(CategoryName::new_unchecked("Foo")).unwrap(),
            store.create(CategoryName::new_unchecked("Bar")).unwrap(),
        ]);

        let selected_categories = store.get_all().unwrap();
        let selected_categories = HashSet::from_iter(selected_categories);

        assert_eq!(inserted_categories, selected_categories);
    }
}
