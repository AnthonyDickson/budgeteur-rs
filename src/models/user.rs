//! This file defines a user of the application and its supporting types.

use std::fmt::Display;

use email_address::EmailAddress;
use rusqlite::{Connection, Row};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    db::{CreateTable, MapRow},
    models::PasswordHash,
};

/// A newtype wrapper for integer user IDs.
/// This helps disambiguate user IDs from other types of IDs, leading to better compile time
/// errors, and more flexible generics that can have distinct implementations for multiple ID types.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct UserID(i64);

impl UserID {
    pub fn new(id: i64) -> Self {
        Self(id)
    }

    pub fn as_i64(&self) -> i64 {
        self.0
    }
}

impl Display for UserID {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
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

/// A user of the application.
///
/// To create a `User` you can either call [User::build] if the user is not in the application
/// database, otherwise you can use [User::select] to retrieve an existing user.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct User {
    id: UserID,
    email: EmailAddress,
    password_hash: PasswordHash,
}

impl User {
    /// Build a new user.
    ///
    /// Shortcut for [UserBuilder::new] for discoverability.
    ///
    /// If you are trying to retrieve an existing user, see [User::select]
    pub fn build(email: EmailAddress, password_hash: PasswordHash) -> UserBuilder {
        UserBuilder::new(email, password_hash)
    }

    /// The user's ID in the database.
    pub fn id(&self) -> UserID {
        self.id
    }

    /// The email address associated with the user.
    pub fn email(&self) -> &EmailAddress {
        &self.email
    }

    /// The user's password hash.
    pub fn password_hash(&self) -> &PasswordHash {
        &self.password_hash
    }

    /// Get the user from the database that has the specified `email` address, or return [UserError::NotFound] if such user does not exist.
    ///
    /// # Examples
    /// ```
    /// use email_address::EmailAddress;
    /// use rusqlite::Connection;
    ///
    /// # use budgeteur_rs::{db::{DbError, SelectBy}, models::User, models::UserError};
    /// #
    /// fn get_user(email: &EmailAddress, connection: &Connection) -> Result<User, UserError> {
    ///     let user = User::select(email, connection)?;
    ///     assert_eq!(user.email(), email);
    ///
    ///     Ok(user)
    /// }
    /// ```
    /// # Panics
    ///
    /// Panics if there is no user with the specified email or there are SQL related errors.
    pub fn select(email: &EmailAddress, connection: &Connection) -> Result<Self, UserError> {
        connection
            .prepare("SELECT id, email, password FROM user WHERE email = :email")?
            .query_row(&[(":email", &email.to_string())], User::map_row)
            .map_err(|e| e.into())
    }
}

impl MapRow for User {
    type ReturnType = Self;

    fn map_row_with_offset(row: &Row, offset: usize) -> Result<Self::ReturnType, rusqlite::Error> {
        let raw_id = row.get(offset)?;
        let raw_email: String = row.get(offset + 1)?;
        let raw_password_hash = row.get(offset + 2)?;

        let id = UserID::new(raw_id);
        let email = EmailAddress::new_unchecked(raw_email);
        let password_hash = PasswordHash::new_unchecked(raw_password_hash);

        Ok(Self {
            id,
            email,
            password_hash,
        })
    }
}

impl CreateTable for User {
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

/// Builder for creating new [User]s.
///
/// The function for finalizing the builder is [UserBuilder::insert].
pub struct UserBuilder {
    email: EmailAddress,
    password_hash: PasswordHash,
}

impl UserBuilder {
    /// Create a new user and insert it into the application database.
    ///
    /// Finalize the builder with [UserBuilder::insert].
    pub fn new(email: EmailAddress, password_hash: PasswordHash) -> Self {
        Self {
            email,
            password_hash,
        }
    }

    /// Insert the user into the application database and return the built user.
    /// Note that this function will consume the builder.
    ///
    /// # Errors
    ///
    /// This function will return a:
    /// - [UserError::DuplicateEmail] if the given email address is already in use,
    /// - [UserError::DuplicatePassword] if the given password hash already exists in the database,
    /// - [UserError::SqlError] if there was an unexpected SQL error.
    pub fn insert(self, connection: &Connection) -> Result<User, UserError> {
        connection.execute(
            "INSERT INTO user (email, password) VALUES (?1, ?2)",
            (&self.email.to_string(), self.password_hash.to_string()),
        )?;

        let id = UserID::new(connection.last_insert_rowid());

        Ok(User {
            id,
            email: self.email,
            password_hash: self.password_hash,
        })
    }
}

#[cfg(test)]
mod user_tests {
    use std::str::FromStr;

    use email_address::EmailAddress;
    use rusqlite::Connection;

    use crate::{
        db::initialize,
        models::{PasswordHash, User, UserError},
    };

    fn init_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();
        conn
    }

    #[test]
    fn insert_user_succeeds() {
        let conn = init_db();

        let email = EmailAddress::from_str("hello@world.com").unwrap();
        let password_hash = PasswordHash::new_unchecked("hunter2".to_string());

        let inserted_user = User::build(email.clone(), password_hash.clone())
            .insert(&conn)
            .unwrap();

        assert!(inserted_user.id().as_i64() > 0);
        assert_eq!(inserted_user.email(), &email);
        assert_eq!(inserted_user.password_hash(), &password_hash);
    }

    #[test]
    fn insert_user_fails_on_duplicate_email() {
        let conn = init_db();

        let email = EmailAddress::from_str("hello@world.com").unwrap();

        assert!(User::build(
            email.clone(),
            PasswordHash::new_unchecked("hunter2".to_string())
        )
        .insert(&conn)
        .is_ok());

        assert_eq!(
            User::build(
                email.clone(),
                PasswordHash::new_unchecked("hunter3".to_string())
            )
            .insert(&conn),
            Err(UserError::DuplicateEmail)
        );
    }

    #[test]
    fn insert_user_fails_on_duplicate_password() {
        let conn = init_db();

        let email = EmailAddress::from_str("hello@world.com").unwrap();
        let password = PasswordHash::new_unchecked("hunter2".to_string());

        assert!(User::build(email, password.clone()).insert(&conn).is_ok());

        assert_eq!(
            User::build(
                EmailAddress::from_str("bye@world.com").unwrap(),
                password.clone()
            )
            .insert(&conn),
            Err(UserError::DuplicatePassword)
        );
    }

    #[test]
    fn select_user_fails_with_non_existent_email() {
        let conn = init_db();

        // This email is not in the database.
        let email = EmailAddress::from_str("notavalidemail@foo.bar").unwrap();

        assert_eq!(User::select(&email, &conn), Err(UserError::NotFound));
    }

    #[test]
    fn select_user_succeeds_with_existing_email() {
        let conn = init_db();

        let test_user = User::build(
            EmailAddress::from_str("foo@bar.baz").unwrap(),
            PasswordHash::new_unchecked("hunter2".to_string()),
        )
        .insert(&conn)
        .unwrap();

        let retrieved_user = User::select(test_user.email(), &conn).unwrap();

        assert_eq!(retrieved_user, test_user);
    }
}
