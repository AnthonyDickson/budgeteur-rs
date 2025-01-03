//! Defines the user store trait and an implentation for the SQLite backend.
use std::sync::{Arc, Mutex};

use email_address::EmailAddress;
use rusqlite::{Connection, Row};
use thiserror::Error;

use crate::{
    db::{CreateTable, MapRow},
    models::{PasswordHash, User, UserID},
};

/// Handles the creation and retrieval of User objects.
pub trait UserStore {
    /// Create a new user.
    fn create(
        &mut self,
        email: EmailAddress,
        password_hash: PasswordHash,
    ) -> Result<User, UserError>;

    /// Get a user by their ID.
    fn get(&self, id: UserID) -> Result<User, UserError>;

    /// Get a user by their email.
    ///
    /// Returns [UserError::NotFound] if no user with the given email exists.
    fn get_by_email(&self, email: &EmailAddress) -> Result<User, UserError>;
}

/// Errors that can occur during the creation or retrieval of a user.
#[derive(Debug, Error, PartialEq)]
pub enum UserError {
    /// The email used to create the user is already in use. The client should try again with a
    /// different email address.
    #[error("the email is already in use")]
    DuplicateEmail,

    /// The password hash already exists in the database. The client should hash the password again.
    #[error("the password hash is not unique")]
    DuplicatePassword,

    /// There was no user in the database that matched the given details. The client can try again
    /// with different details.
    #[error("no user found with the given details")]
    NotFound,

    /// An unhandled/unexpected SQL error.
    #[error("an error occurred while creating the user: {0}")]
    SqlError(rusqlite::Error),
}

impl From<rusqlite::Error> for UserError {
    fn from(value: rusqlite::Error) -> Self {
        match value {
            // Code 2067 occurs when a UNIQUE constraint failed.
            rusqlite::Error::SqliteFailure(sql_error, Some(ref desc))
                if sql_error.extended_code == 2067 && desc.contains("email") =>
            {
                UserError::DuplicateEmail
            }
            rusqlite::Error::SqliteFailure(sql_error, Some(ref desc))
                if sql_error.extended_code == 2067 && desc.contains("password") =>
            {
                UserError::DuplicatePassword
            }
            rusqlite::Error::QueryReturnedNoRows => UserError::NotFound,
            error => UserError::SqlError(error),
        }
    }
}

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
    /// Returns a [UserError::SqlError] if an SQL related error occurred.
    fn create(
        &mut self,
        email: EmailAddress,
        password_hash: PasswordHash,
    ) -> Result<User, UserError> {
        let connection = self.connection.lock().unwrap();

        connection.execute(
            "INSERT INTO user (email, password) VALUES (?1, ?2)",
            (&email.to_string(), password_hash.to_string()),
        )?;

        let id = UserID::new(connection.last_insert_rowid());

        Ok(User::new(id, email, password_hash))
    }

    /// Get the user from the database that has the specified `id`, or return [UserError::NotFound] if such user does not exist.
    ///
    /// # Panics
    ///
    /// Panics if the database lock is already acquired by the same thread or is poisoned.
    ///
    /// # Errors
    ///
    /// Returns a [UserError::NotFound] error if there is no user with the specified email or [UserError::SqlError] if there are SQL related errors.
    fn get(&self, id: UserID) -> Result<User, UserError> {
        self.connection
            .lock()
            .unwrap()
            .prepare("SELECT id, email, password FROM user WHERE id = :id")?
            .query_row(&[(":id", &id.as_i64())], SQLiteUserStore::map_row)
            .map_err(|e| e.into())
    }

    /// Get the user from the database that has the specified `email` address, or return [UserError::NotFound] if such user does not exist.
    ///
    /// # Panics
    ///
    /// Panics if the database lock is already acquired by the same thread or is poisoned.
    ///
    /// # Errors
    ///
    /// Returns a [UserError::NotFound] error if there is no user with the specified email or [UserError::SqlError] there are SQL related errors.
    fn get_by_email(&self, email: &EmailAddress) -> Result<User, UserError> {
        self.connection
            .lock()
            .unwrap()
            .prepare("SELECT id, email, password FROM user WHERE email = :email")?
            .query_row(&[(":email", &email.to_string())], SQLiteUserStore::map_row)
            .map_err(|e| e.into())
    }
}

impl CreateTable for SQLiteUserStore {
    fn create_table(connection: &Connection) -> Result<(), rusqlite::Error> {
        connection.execute(
            "CREATE TABLE user (
                    id INTEGER PRIMARY KEY,
                    email TEXT UNIQUE NOT NULL,
                    password TEXT UNIQUE NOT NULL
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

    use super::{SQLiteUserStore, UserError, UserStore};

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

        assert!(store
            .create(email.clone(), PasswordHash::new_unchecked("hunter2"))
            .is_ok());

        assert_eq!(
            store.create(email.clone(), PasswordHash::new_unchecked("hunter3")),
            Err(UserError::DuplicateEmail)
        );
    }

    #[test]
    fn insert_user_fails_on_duplicate_password() {
        let mut store = get_store();

        let email = EmailAddress::from_str("hello@world.com").unwrap();
        let password = PasswordHash::new_unchecked("hunter2");

        assert!(store.create(email, password.clone()).is_ok());

        assert_eq!(
            store.create(
                EmailAddress::from_str("bye@world.com").unwrap(),
                password.clone()
            ),
            Err(UserError::DuplicatePassword)
        );
    }

    #[test]
    fn get_user_fails_with_non_existent_id() {
        let store = get_store();

        let id = UserID::new(42);

        assert_eq!(store.get(id), Err(UserError::NotFound));
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

        assert_eq!(store.get_by_email(&email), Err(UserError::NotFound));
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
}
