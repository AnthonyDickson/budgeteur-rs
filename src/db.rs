use chrono::NaiveDate;
use rusqlite::{Connection, Error};
use serde::{Deserialize, Serialize};

pub trait Model {
    /// Get the SQL query string to create a table for the model.
    fn create_table_sql() -> &'static str;
}

type DatabaseID = i64;

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct User {
    id: DatabaseID,
    email: String,
    password: String,
}

impl User {
    pub fn new(id: DatabaseID, email: String, password: String) -> User {
        User {
            id,
            email,
            password,
        }
    }

    pub fn id(&self) -> DatabaseID {
        self.id
    }
    pub fn email(&self) -> &str {
        &self.email
    }
    /// Most likely the hashed password.
    pub fn password(&self) -> &str {
        &self.password
    }

    /// Get a user from the database that has the specified `email` address or `None` if the user does not exist.
    ///
    /// # Examples
    /// ```
    /// use rusqlite::Connection;
    ///
    /// use backrooms_rs::db::{Model, User};
    ///
    /// let conn = Connection::open_in_memory().unwrap();
    /// conn.execute(User::create_table_sql(), ());
    ///
    /// let inserted_user = User::insert("foo@bar.baz", "hunter2", &conn).unwrap();
    /// let selected_user = User::select(inserted_user.email(), &conn).unwrap();
    ///
    /// assert_eq!(inserted_user, selected_user);
    /// ```
    /// # Panics
    ///
    /// Panics if there are SQL related errors.
    pub fn select(email: &str, db_connection: &Connection) -> Result<User, DbError> {
        let mut stmt = db_connection
            .prepare("SELECT id, email, password FROM user WHERE email = :email")
            .map_err(DbError::SqlError)?;

        let rows = stmt
            .query_map(&[(":email", &email)], |row| {
                let id: i64 = row.get(0)?;
                let email: String = row.get(1)?;
                let password: String = row.get(2)?;

                Ok(User::new(id, email, password))
            })
            .map_err(DbError::SqlError)?;

        let row = rows.into_iter().next();

        match row {
            Some(user_result) => user_result.map_err(DbError::SqlError),
            None => Err(DbError::EmailNotFound),
        }
    }

    /// Create a new user in the database.
    ///
    /// It is up to the caller to ensure the password is properly hashed.
    ///
    /// # Error
    /// Will return an error if there was a problem executing the SQL query. This could be due to:
    /// - a syntax error in the SQL string,
    /// - the email is already in use, or
    /// - the password hash is not unique.
    pub fn insert(
        email: &str,
        password_hash: &str,
        connection: &Connection,
    ) -> Result<User, DbError> {
        // TODO: Check for invalid email format.
        if email.is_empty() {
            return Err(DbError::EmptyEmail);
        }

        if password_hash.is_empty() {
            return Err(DbError::EmptyPassword);
        }

        connection
            .execute(
                "INSERT INTO user (email, password) VALUES (?1, ?2)",
                (email, password_hash),
            )
            .map_err(|e| match e {
                Error::SqliteFailure(error, Some(ref desc)) if error.extended_code == 2067 => {
                    if desc.contains("email") {
                        DbError::DuplicateEmail
                    } else if desc.contains("password") {
                        DbError::DuplicatePassword
                    } else {
                        DbError::SqlError(e)
                    }
                }
                _ => DbError::SqlError(e),
            })?;

        let id = connection.last_insert_rowid();

        Ok(User::new(id, email.to_string(), password_hash.to_string()))
    }
}

impl Model for User {
    fn create_table_sql() -> &'static str {
        "CREATE TABLE user (
                    id INTEGER PRIMARY KEY,
                    email TEXT UNIQUE NOT NULL,
                    password TEXT UNIQUE NOT NULL
                    )"
    }
}

struct Category {
    id: DatabaseID,
    name: String,
}

impl Model for Category {
    fn create_table_sql() -> &'static str {
        "CREATE TABLE category (
                id INTEGER PRIMARY KEY,
                name TEXT UNIQUE NOT NULL
                )"
    }
}

struct Transaction {
    id: DatabaseID,
    amount: f64,
    date: NaiveDate,
    description: String,
    category_id: DatabaseID,
    user_id: DatabaseID,
}

impl Model for Transaction {
    fn create_table_sql() -> &'static str {
        "CREATE TABLE \"transaction\" (
                id INTEGER PRIMARY KEY,
                amount REAL NOT NULL,
                date TEXT NOT NULL,
                description TEXT NOT NULL,
                category_id INTEGER,
                user_id INTEGER NOT NULL,
                FOREIGN KEY(category_id) REFERENCES category(id) ON UPDATE CASCADE ON DELETE CASCADE,
                FOREIGN KEY(user_id) REFERENCES user(id) ON UPDATE CASCADE ON DELETE CASCADE
                )"
    }
}

struct SavingsRatio {
    transaction_id: DatabaseID,
    ratio: f64,
}

impl Model for SavingsRatio {
    fn create_table_sql() -> &'static str {
        "CREATE TABLE savings_ratio (
                transaction_id INTEGER PRIMARY KEY,
                ratio REAL NOT NULL,
                FOREIGN KEY(transaction_id) REFERENCES \"transaction\"(id) ON UPDATE CASCADE ON DELETE CASCADE
                )"
    }
}

struct RecurringTransaction {
    transaction_id: DatabaseID,
    start_date: NaiveDate,
    end_date: NaiveDate,
    frequency: i64,
}

impl Model for RecurringTransaction {
    fn create_table_sql() -> &'static str {
        "CREATE TABLE recurring_transaction (
                transaction_id INTEGER PRIMARY KEY,
                start_date TEXT NOT NULL,
                end_date TEXT NOT NULL,
                frequency INTEGER NOT NULL,
                FOREIGN KEY(transaction_id) REFERENCES \"transaction\"(id) ON UPDATE CASCADE ON DELETE CASCADE
                )"
    }
}

pub fn initialize(connection: &Connection) -> Result<(), Error> {
    connection.execute(User::create_table_sql(), ())?;
    connection.execute(Category::create_table_sql(), ())?;
    connection.execute(Transaction::create_table_sql(), ())?;
    connection.execute(SavingsRatio::create_table_sql(), ())?;
    connection.execute(RecurringTransaction::create_table_sql(), ())?;

    Ok(())
}

/// Errors originating from operations on the app's database.
#[derive(Debug, PartialEq)]
pub enum DbError {
    /// The specified email address could not be found in the database. The client could try again with a different email address.
    EmailNotFound,
    /// An empty email was given. The client should try again with a non-empty email address.
    EmptyEmail,
    /// An empty password hash was given. The client should try again with a non-empty password hash.
    EmptyPassword,
    /// The user's email already exists in the database. The client should try again with a different email address.
    DuplicateEmail,
    /// The password hash clashed with an existing password hash (should be extremely rare), the caller should rehash the password and try again.
    DuplicatePassword,
    /// Wrapper for Sqlite errors not handled by the other enum entries.
    SqlError(Error),
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use crate::db::{initialize, DbError, User};

    #[test]
    fn create_user() {
        let conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();

        let email = "hello@world.com";
        let password = "hunter2";

        let inserted_user = User::insert(email, password, &conn).unwrap();

        assert!(inserted_user.id > 0);
        assert_eq!(inserted_user.email, email);
        assert_eq!(inserted_user.password, password);
    }

    #[test]
    fn create_user_empty_email() {
        let conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();

        assert_eq!(User::insert("", "hunter2", &conn), Err(DbError::EmptyEmail));
    }

    #[test]
    fn create_user_empty_password() {
        let conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();

        assert_eq!(
            User::insert("foo@bar.baz", "", &conn),
            Err(DbError::EmptyPassword)
        );
    }

    #[test]
    fn create_user_duplicate_email() {
        let conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();

        let email = "hello@world.com";
        let password = "hunter2";

        assert!(User::insert(email, password, &conn).is_ok());
        assert_eq!(
            User::insert(email, "hunter3", &conn),
            Err(DbError::DuplicateEmail)
        );
    }

    #[test]
    fn create_user_duplicate_password() {
        let conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();

        let email = "hello@world.com";
        let password = "hunter2";

        assert!(User::insert(email, password, &conn).is_ok());
        assert_eq!(
            User::insert("bye@world.com", password, &conn),
            Err(DbError::DuplicatePassword)
        );
    }

    #[test]
    fn select_user_by_non_existent_email() {
        let conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();
        User::insert("foo@bar.baz", "hunter2", &conn).unwrap();

        let email = "notavalidemail";

        assert_eq!(User::select(email, &conn), Err(DbError::EmailNotFound));
    }

    #[test]
    fn select_user_by_existing_email() {
        let conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();

        let test_user = User::insert("foo@bar.baz", "hunter2", &conn).unwrap();
        let retrieved_user = User::select(test_user.email(), &conn).unwrap();

        assert_eq!(retrieved_user, test_user);
    }
}
