use std::sync::{Arc, Mutex};

use rusqlite::{Connection, Row};

use crate::{
    db::{CreateTable, MapRow},
    models::{Category, CategoryError, CategoryName, DatabaseID, UserID},
};

pub trait CategoryStore {
    fn create(&self, name: CategoryName, user_id: UserID) -> Result<Category, CategoryError>;
    fn select(&self, category_id: DatabaseID) -> Result<Category, CategoryError>;
    fn select_by_user(&self, user_id: UserID) -> Result<Vec<Category>, CategoryError>;
}

#[derive(Debug, Clone)]
pub struct SQLiteCategoryStore {
    connection: Arc<Mutex<Connection>>,
}

impl SQLiteCategoryStore {
    pub fn new(connection: Arc<Mutex<Connection>>) -> Self {
        Self { connection }
    }
}

impl CategoryStore for SQLiteCategoryStore {
    fn create(&self, name: CategoryName, user_id: UserID) -> Result<Category, CategoryError> {
        let connection = self.connection.lock().unwrap();
        connection.execute(
            "INSERT INTO category (name, user_id) VALUES (?1, ?2)",
            (name.as_ref(), user_id.as_i64()),
        )?;

        let id = connection.last_insert_rowid();

        Ok(Category::new(id, name, user_id))
    }

    /// Retrieve categories in the database for the user `user_id`.
    ///
    /// # Errors
    /// This function will return an error if there is an SQL error.
    fn select(&self, category_id: DatabaseID) -> Result<Category, CategoryError> {
        self.connection
            .lock()
            .unwrap()
            .prepare("SELECT id, name, user_id FROM category WHERE id = :id")?
            .query_row(&[(":id", &category_id)], Category::map_row)
            .map_err(|error| error.into())
    }

    /// Retrieve categories in the database for the user `user_id`.
    ///
    /// # Errors
    /// This function will return an error if there is an SQL error.
    fn select_by_user(&self, user_id: UserID) -> Result<Vec<Category>, CategoryError> {
        self.connection
            .lock()
            .unwrap()
            .prepare("SELECT id, name, user_id FROM category WHERE user_id = :user_id")?
            .query_map(&[(":user_id", &user_id.as_i64())], Category::map_row)?
            .map(|maybe_category| maybe_category.map_err(CategoryError::SqlError))
            .collect()
    }
}

impl CreateTable for SQLiteCategoryStore {
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
        let name = CategoryName::new_unchecked(&raw_name);

        let raw_user_id = row.get(offset + 2)?;
        let user_id = UserID::new(raw_user_id);

        Ok(Self::new(id, name, user_id))
    }
}

/// TODO: Make similar tests for routes module
#[cfg(test)]
mod category_tests {
    use std::collections::HashSet;
    use std::sync::{Arc, Mutex};

    use rusqlite::Connection;

    use crate::db::initialize;
    use crate::models::{
        CategoryError, CategoryName, PasswordHash, User, UserID, ValidatedPassword,
    };

    use super::{CategoryStore, SQLiteCategoryStore};

    fn get_store_and_user() -> (SQLiteCategoryStore, User) {
        let connection = Connection::open_in_memory().unwrap();
        initialize(&connection).unwrap();

        let user = User::build(
            "foo@bar.baz".parse().unwrap(),
            PasswordHash::new(ValidatedPassword::new_unchecked("foo".to_string())).unwrap(),
        )
        .insert(&connection)
        .unwrap();

        let store = SQLiteCategoryStore::new(Arc::new(Mutex::new(connection)));

        (store, user)
    }

    #[test]
    fn create_category_succeeds() {
        let (store, user) = get_store_and_user();
        let name = CategoryName::new("Categorically a category").unwrap();

        let category = store.create(name.clone(), user.id()).unwrap();

        assert!(category.id() > 0);
        assert_eq!(category.name(), &name);
        assert_eq!(category.user_id(), user.id());
    }

    #[test]
    fn select_category_succeeds() {
        let (store, user) = get_store_and_user();

        let name = CategoryName::new_unchecked("Foo");
        let inserted_category = store.create(name, user.id()).unwrap();

        let selected_category = store.select(inserted_category.id());

        assert_eq!(Ok(inserted_category), selected_category);
    }

    #[test]
    fn select_category_with_invalid_id_returns_not_found() {
        let (store, user) = get_store_and_user();
        let inserted_category = store
            .create(CategoryName::new_unchecked("Foo"), user.id())
            .unwrap();

        let selected_category = store.select(inserted_category.id() + 123);

        assert_eq!(selected_category, Err(CategoryError::NotFound));
    }

    #[test]
    fn select_category_with_user_id() {
        let (store, user) = get_store_and_user();

        let inserted_categories = HashSet::from([
            store
                .create(CategoryName::new_unchecked("Foo"), user.id())
                .unwrap(),
            store
                .create(CategoryName::new_unchecked("Bar"), user.id())
                .unwrap(),
        ]);

        let selected_categories = store.select_by_user(user.id()).unwrap();
        let selected_categories = HashSet::from_iter(selected_categories);

        assert_eq!(inserted_categories, selected_categories);
    }

    #[test]
    fn select_category_with_invalid_user_id() {
        let (store, user) = get_store_and_user();

        store
            .create(CategoryName::new_unchecked("Foo"), user.id())
            .unwrap();
        store
            .create(CategoryName::new_unchecked("Bar"), user.id())
            .unwrap();

        let selected_categories = store.select_by_user(UserID::new(user.id().as_i64() + 123));

        assert_eq!(selected_categories, Ok(vec![]));
    }
}
