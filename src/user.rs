//! Code for creating the user table and fetching users from the database.

use email_address::EmailAddress;
use rusqlite::Connection;

use crate::{Error, models::PasswordHash};
use std::fmt::Display;

use serde::{Deserialize, Serialize};

/// A newtype wrapper for integer user IDs.
///
/// This helps disambiguate user IDs from other types of IDs, leading to better compile time
/// errors, and more flexible generics that can have distinct implementations for multiple ID types.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct UserID(i64);

impl UserID {
    /// Create a new user ID.
    pub fn new(id: i64) -> Self {
        Self(id)
    }

    /// Cast the user ID to a 64 bit integer.
    pub fn as_i64(&self) -> i64 {
        self.0
    }
}

impl Display for UserID {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// A user of the application.
///
/// The caller should ensure that `id` is unique.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct User {
    /// The user's ID in the application database.
    pub id: UserID,
    /// The email address associated with the user.
    pub email: EmailAddress,
    /// The user's password hash.
    pub password_hash: PasswordHash,
}

impl User {
    /// Create a new user.
    ///
    /// The caller should ensure that `id` is unique.
    pub fn new(id: UserID, email: EmailAddress, password_hash: PasswordHash) -> Self {
        Self {
            id,
            email,
            password_hash,
        }
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
}

/// Create the user table.
///
/// # Errors
///
/// This function will return an error if the SQL query failed.
pub fn create_user_table(connection: &Connection) -> Result<(), rusqlite::Error> {
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

/// Create and insert a new user into the database.
///
/// # Errors
///
/// Returns a [Error::SqlError] if an SQL related error occurred.
pub fn create_user(
    email: EmailAddress,
    password_hash: PasswordHash,
    connection: &Connection,
) -> Result<User, Error> {
    connection.execute(
        "INSERT INTO user (email, password) VALUES (?1, ?2)",
        (email.as_str(), password_hash.as_ref()),
    )?;

    let id = UserID::new(connection.last_insert_rowid());

    Ok(User::new(id, email, password_hash))
}

/// Get the user from the database with an ID equal to `user_id`.
///
/// # Errors
///
/// This function will return an error if:
/// - `user_id` does not belong to a registered user.
/// - there was an error trying to access the store.
pub fn get_user_by_id(user_id: UserID, db_connection: &Connection) -> Result<User, Error> {
    db_connection
        .prepare("SELECT id, email, password FROM user WHERE id = :id")?
        .query_row(&[(":id", &user_id.as_i64())], |row| {
            let raw_id = row.get(0)?;
            let raw_email: String = row.get(1)?;
            let raw_password_hash: String = row.get(2)?;

            let id = UserID::new(raw_id);
            let email = EmailAddress::new_unchecked(raw_email);
            let password_hash = PasswordHash::new_unchecked(&raw_password_hash);

            Ok(User {
                id,
                email,
                password_hash,
            })
        })
        .map_err(|error| error.into())
}

/// Get the user from the database with an email address equal to `email`.
///
/// # Errors
///
/// This function will return an error if:
/// - the email does not belong to a registered user.
/// - there was an error trying to access the store.
pub fn get_user_by_email(email: &str, db_connection: &Connection) -> Result<User, Error> {
    db_connection
        .prepare("SELECT id, email, password FROM user WHERE email = :email")?
        .query_row(&[(":email", email)], |row| {
            let raw_id = row.get(0)?;
            let raw_email: String = row.get(1)?;
            let raw_password_hash: String = row.get(2)?;

            let id = UserID::new(raw_id);
            let email = EmailAddress::new_unchecked(raw_email);
            let password_hash = PasswordHash::new_unchecked(&raw_password_hash);

            Ok(User {
                id,
                email,
                password_hash,
            })
        })
        .map_err(|error| error.into())
}

/// Get the number of users in the database.
///
/// # Errors
///
/// Returns a [Error::SqlError] if an SQL related error occurred.
pub fn count_users(connection: &Connection) -> Result<usize, Error> {
    connection
        .query_row("SELECT COUNT(id) FROM user;", [], |row| row.get(0))
        .map_err(|error| error.into())
}

#[cfg(test)]
mod user_tests {
    use std::str::FromStr;

    use email_address::EmailAddress;
    use rusqlite::Connection;

    use crate::{
        models::PasswordHash,
        user::{UserID, count_users, create_user, get_user_by_email, get_user_by_id},
    };

    use super::{Error, create_user_table};

    fn get_db_connection() -> Connection {
        let conn =
            Connection::open_in_memory().expect("Could not create in-memory SQLite database");
        create_user_table(&conn).expect("Could not create user table");

        conn
    }

    #[test]
    fn insert_user_succeeds() {
        let db_connection = get_db_connection();

        let email = EmailAddress::from_str("hello@world.com").unwrap();
        let password_hash = PasswordHash::new_unchecked("hunter2");

        let inserted_user =
            create_user(email.clone(), password_hash.clone(), &db_connection).unwrap();

        assert!(inserted_user.id().as_i64() > 0);
        assert_eq!(inserted_user.email(), &email);
        assert_eq!(inserted_user.password_hash(), &password_hash);
    }

    #[test]
    fn insert_user_fails_on_duplicate_email() {
        let db_connection = get_db_connection();

        let email = EmailAddress::from_str("hello@world.com").unwrap();

        assert!(
            create_user(
                email.clone(),
                PasswordHash::new_unchecked("hunter2"),
                &db_connection
            )
            .is_ok()
        );

        assert_eq!(
            create_user(
                email.clone(),
                PasswordHash::new_unchecked("hunter3"),
                &db_connection
            ),
            Err(Error::DuplicateEmail)
        );
    }

    #[test]
    fn get_user_fails_with_non_existent_id() {
        let db_connection = get_db_connection();

        let id = UserID::new(42);

        assert_eq!(get_user_by_id(id, &db_connection), Err(Error::NotFound));
    }

    #[test]
    fn get_user_succeeds_with_existing_id() {
        let db_connection = get_db_connection();

        let test_user = create_user(
            EmailAddress::from_str("foo@bar.baz").unwrap(),
            PasswordHash::new_unchecked("hunter2"),
            &db_connection,
        )
        .unwrap();

        let retrieved_user = get_user_by_id(test_user.id(), &db_connection).unwrap();

        assert_eq!(retrieved_user, test_user);
    }

    #[test]
    fn get_user_fails_with_non_existent_email() {
        let db_connection = get_db_connection();

        // This email is not in the database.
        let email = EmailAddress::from_str("notavalidemail@foo.bar").unwrap();

        assert_eq!(
            get_user_by_email(email.as_str(), &db_connection),
            Err(Error::NotFound)
        );
    }

    #[test]
    fn get_user_succeeds_with_existing_email() {
        let db_connection = get_db_connection();
        let test_user = create_user(
            EmailAddress::from_str("foo@bar.baz").unwrap(),
            PasswordHash::new_unchecked("hunter2"),
            &db_connection,
        )
        .unwrap();

        let retrieved_user = get_user_by_email(test_user.email.as_str(), &db_connection).unwrap();

        assert_eq!(retrieved_user, test_user);
    }

    #[test]
    fn returns_correct_count() {
        let db_connection = get_db_connection();

        let count = count_users(&db_connection).expect("Could not get user count");
        assert_eq!(0, count, "Want zero users before insertion, got {count}");

        create_user(
            EmailAddress::from_str("foo@bar.baz").unwrap(),
            PasswordHash::new_unchecked("hunter2"),
            &db_connection,
        )
        .unwrap();

        let count = count_users(&db_connection).expect("Could not get user count");
        assert_eq!(1, count, "Want one user after insertion, got {count}");
    }
}
