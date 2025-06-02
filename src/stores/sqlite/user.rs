//! Implements a SQLite backed user store.
use std::sync::{Arc, Mutex};

use email_address::EmailAddress;
use rusqlite::{Connection, Row};

use crate::{
    Error,
    db::{CreateTable, MapRow},
    models::{PasswordHash, User, UserID},
    stores::UserStore,
};

/// Handles the creation and retrieval of User objects.
#[derive(Debug, Clone)]
pub struct SQLiteUserStore {
    connection: Arc<Mutex<Connection>>,
}

impl SQLiteUserStore {
    /// Create a new user store.
    pub fn new(connection: Arc<Mutex<Connection>>) -> Self {
        Self { connection }
    }
}

impl UserStore for SQLiteUserStore {
    /// Create and insert a new user into the database.
    ///
    /// # Panics
    ///
    /// Panics if the database lock is already acquired by the same thread or is poisoned.
    ///
    /// # Errors
    ///
    /// Returns a [Error::SqlError] if an SQL related error occurred.
    fn create(&mut self, email: EmailAddress, password_hash: PasswordHash) -> Result<User, Error> {
        let connection = self.connection.lock().unwrap();

        connection.execute(
            "INSERT INTO user (email, password) VALUES (?1, ?2)",
            (&email.to_string(), password_hash.to_string()),
        )?;

        let id = UserID::new(connection.last_insert_rowid());

        Ok(User::new(id, email, password_hash))
    }

    /// Get the user from the database that has the specified `id`, or return [Error::NotFound] if such user does not exist.
    ///
    /// # Panics
    ///
    /// Panics if the database lock is already acquired by the same thread or is poisoned.
    ///
    /// # Errors
    ///
    /// Returns a [Error::NotFound] error if there is no user with the specified email or [Error::SqlError] if there are SQL related errors.
    fn get(&self, id: UserID) -> Result<User, Error> {
        self.connection
            .lock()
            .unwrap()
            .prepare("SELECT id, email, password FROM user WHERE id = :id")?
            .query_row(&[(":id", &id.as_i64())], SQLiteUserStore::map_row)
            .map_err(|e| e.into())
    }

    /// Get the user from the database that has the specified `email` address, or return [Error::NotFound] if such user does not exist.
    ///
    /// # Panics
    ///
    /// Panics if the database lock is already acquired by the same thread or is poisoned.
    ///
    /// # Errors
    ///
    /// Returns a [Error::NotFound] error if there is no user with the specified email or [Error::SqlError] there are SQL related errors.
    fn get_by_email(&self, email: &EmailAddress) -> Result<User, Error> {
        self.connection
            .lock()
            .unwrap()
            .prepare("SELECT id, email, password FROM user WHERE email = :email")?
            .query_row(&[(":email", &email.to_string())], SQLiteUserStore::map_row)
            .map_err(|e| e.into())
    }

    fn count(&self) -> Result<usize, Error> {
        self.connection
            .lock()
            .unwrap()
            .query_row("SELECT COUNT(id) FROM user;", [], |row| row.get(0))
            .map_err(|error| error.into())
    }
}

impl CreateTable for SQLiteUserStore {
    fn create_table(connection: &Connection) -> Result<(), rusqlite::Error> {
        connection.execute(
            "CREATE TABLE IF NOT EXISTS user (
                    id INTEGER PRIMARY KEY,
                    email TEXT UNIQUE NOT NULL,
                    password TEXT NOT NULL
                    )",
            (),
        )?;

        Ok(())
    }
}

impl MapRow for SQLiteUserStore {
    type ReturnType = User;

    fn map_row_with_offset(row: &Row, offset: usize) -> Result<Self::ReturnType, rusqlite::Error> {
        let raw_id = row.get(offset)?;
        let raw_email: String = row.get(offset + 1)?;
        let raw_password_hash: String = row.get(offset + 2)?;

        let id = UserID::new(raw_id);
        let email = EmailAddress::new_unchecked(raw_email);
        let password_hash = PasswordHash::new_unchecked(&raw_password_hash);

        Ok(Self::ReturnType::new(id, email, password_hash))
    }
}

#[cfg(test)]
mod user_tests {
    use std::{
        str::FromStr,
        sync::{Arc, Mutex},
    };

    use email_address::EmailAddress;
    use rusqlite::Connection;

    use crate::{
        db::CreateTable,
        models::{PasswordHash, UserID},
    };

    use super::{Error, SQLiteUserStore, UserStore};

    fn get_store() -> SQLiteUserStore {
        let conn = Connection::open_in_memory().unwrap();
        SQLiteUserStore::create_table(&conn).unwrap();

        SQLiteUserStore::new(Arc::new(Mutex::new(conn)))
    }

    #[test]
    fn insert_user_succeeds() {
        let mut store = get_store();

        let email = EmailAddress::from_str("hello@world.com").unwrap();
        let password_hash = PasswordHash::new_unchecked("hunter2");

        let inserted_user = store.create(email.clone(), password_hash.clone()).unwrap();

        assert!(inserted_user.id().as_i64() > 0);
        assert_eq!(inserted_user.email(), &email);
        assert_eq!(inserted_user.password_hash(), &password_hash);
    }

    #[test]
    fn insert_user_fails_on_duplicate_email() {
        let mut store = get_store();

        let email = EmailAddress::from_str("hello@world.com").unwrap();

        assert!(
            store
                .create(email.clone(), PasswordHash::new_unchecked("hunter2"))
                .is_ok()
        );

        assert_eq!(
            store.create(email.clone(), PasswordHash::new_unchecked("hunter3")),
            Err(Error::DuplicateEmail)
        );
    }

    #[test]
    fn get_user_fails_with_non_existent_id() {
        let store = get_store();

        let id = UserID::new(42);

        assert_eq!(store.get(id), Err(Error::NotFound));
    }

    #[test]
    fn get_user_succeeds_with_existing_id() {
        let mut store = get_store();

        let test_user = store
            .create(
                EmailAddress::from_str("foo@bar.baz").unwrap(),
                PasswordHash::new_unchecked("hunter2"),
            )
            .unwrap();

        let retrieved_user = store.get(test_user.id()).unwrap();

        assert_eq!(retrieved_user, test_user);
    }

    #[test]
    fn get_user_fails_with_non_existent_email() {
        let store = get_store();

        // This email is not in the database.
        let email = EmailAddress::from_str("notavalidemail@foo.bar").unwrap();

        assert_eq!(store.get_by_email(&email), Err(Error::NotFound));
    }

    #[test]
    fn get_user_succeeds_with_existing_email() {
        let mut store = get_store();
        let test_user = store
            .create(
                EmailAddress::from_str("foo@bar.baz").unwrap(),
                PasswordHash::new_unchecked("hunter2"),
            )
            .unwrap();

        let retrieved_user = store.get_by_email(test_user.email()).unwrap();

        assert_eq!(retrieved_user, test_user);
    }

    #[test]
    fn returns_correct_count() {
        let mut store = get_store();

        let count = store.count().expect("Could not get user count");
        assert_eq!(0, count, "Want zero users before insertion, got {count}");

        store
            .create(
                EmailAddress::from_str("foo@bar.baz").unwrap(),
                PasswordHash::new_unchecked("hunter2"),
            )
            .unwrap();

        let count = store.count().expect("Could not get user count");
        assert_eq!(1, count, "Want one user after insertion, got {count}");
    }
}
