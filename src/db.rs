use chrono::NaiveDate;
use rusqlite::{Connection, Error, Transaction as SqlTransaction};
use serde::{Deserialize, Serialize};

/// Errors originating from operations on the app's database.
#[derive(Debug, PartialEq)]
pub enum DbError {
    /// An empty email was given. The client should try again with a non-empty email address.
    EmptyEmail,
    /// An empty password hash was given. The client should try again with a non-empty password hash.
    EmptyPassword,
    /// The user's email already exists in the database. The client should try again with a different email address.
    DuplicateEmail,
    /// The password hash clashed with an existing password hash (should be extremely rare), the caller should rehash the password and try again.
    DuplicatePassword,
    /// The specified field was empty when it is not allowed. The client should try again after replacing the field with a non-empty string.
    EmptyField(String),
    /// A query was given an invalid foreign key. The client should try again with a valid foreign key.
    InvalidForeignKey(String),
    /// The row could not be found with the provided info (e.g., id). The client should try again with different parameters.
    NotFound,
    /// Wrapper for Sqlite errors not handled by the other enum entries.
    SqlError(Error),
}

pub trait Model {
    /// Create a table for the model.
    ///
    /// # Errors
    /// Returns an error if the table already exists or if there is an SQL error.
    fn create_table(connection: &Connection) -> Result<(), DbError>;
}

type DatabaseID = i64;

/// A user of the application.
///
/// New instances should be created through `User::insert(...)`.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct User {
    id: DatabaseID,
    email: String,
    password: String,
}

impl User {
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

    /// Create a new user in the database.
    ///
    /// It is up to the caller to ensure the password is properly hashed.
    ///
    /// # Error
    /// Will return an error if there was a problem executing the SQL query. This could be due to:
    /// - the email is empty,
    /// - the password is empty,
    /// - a syntax error in the SQL string,
    /// - the email is already in use, or
    /// - the password hash is not unique.
    pub fn insert(
        email: String,
        password_hash: String,
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
                (&email, &password_hash),
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

        Ok(User {
            id,
            email,
            password: password_hash,
        })
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
    /// User::create_table(&conn).unwrap();
    /// let inserted_user = User::insert("foo@bar.baz".to_string(), "hunter2".to_string(), &conn).unwrap();
    ///
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

        stmt.query_row(&[(":email", &email)], |row| {
            let id: i64 = row.get(0)?;
            let email: String = row.get(1)?;
            let password: String = row.get(2)?;

            Ok(User {
                id,
                email,
                password,
            })
        })
        .map_err(|e| match e {
            Error::QueryReturnedNoRows => DbError::NotFound,
            e => DbError::SqlError(e),
        })
    }
}

impl Model for User {
    fn create_table(connection: &Connection) -> Result<(), DbError> {
        connection
            .execute(
                "CREATE TABLE user (
                    id INTEGER PRIMARY KEY,
                    email TEXT UNIQUE NOT NULL,
                    password TEXT UNIQUE NOT NULL
                    )",
                (),
            )
            .map_err(DbError::SqlError)?;

        Ok(())
    }
}

/// A category for expenses and income, e.g., 'Groceries', 'Eating Out', 'Wages'.
///
/// New instances should be created through `Category::insert(...)`.
#[derive(Debug, PartialEq)]
pub struct Category {
    id: DatabaseID,
    name: String,
    user_id: DatabaseID,
}

impl Category {
    /// The id of the category.
    pub fn id(&self) -> DatabaseID {
        self.id
    }

    /// The name of the category.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The id of the user that created the category.
    pub fn user_id(&self) -> DatabaseID {
        self.user_id
    }

    /// Create a new category in the database.
    ///
    /// # Examples
    /// ```
    /// use rusqlite::Connection;
    ///
    /// use backrooms_rs::db::{Model, User, Category};
    ///
    /// let conn = Connection::open_in_memory().unwrap();
    /// User::create_table(&conn).unwrap();
    /// Category::create_table(&conn).unwrap();
    ///
    /// let test_user = User::insert("foo@bar.baz".to_string(), "hunter2".to_string(), &conn).unwrap();
    /// let inserted_category = Category::insert("foo".to_string(), test_user.id(), &conn).unwrap();
    ///
    /// assert_eq!(inserted_category.name(), "foo");
    /// assert_eq!(inserted_category.user_id(), test_user.id());
    /// ```
    ///
    /// # Errors
    /// Will return an error if:
    /// - `name` is empty,
    /// - `user_id` does not refer to a valid user,
    /// - or there is some other SQL error.
    pub fn insert(
        name: String,
        user_id: DatabaseID,
        connection: &Connection,
    ) -> Result<Category, DbError> {
        if name.is_empty() {
            return Err(DbError::EmptyField(name));
        }

        connection
            .execute(
                "INSERT INTO category (name, user_id) VALUES (?1, ?2)",
                (&name, user_id),
            )
            .map_err(|e| match e {
                Error::SqliteFailure(error, Some(_)) if error.extended_code == 787 => {
                    DbError::InvalidForeignKey("user_id".to_string())
                }
                _ => DbError::SqlError(e),
            })?;

        let category_id = connection.last_insert_rowid();

        Ok(Category {
            id: category_id,
            name,
            user_id,
        })
    }

    /// Retrieve a category in the database by its `id`.
    ///
    /// # Examples
    /// ```
    /// use rusqlite::Connection;
    ///
    /// use backrooms_rs::db::{Model, User, Category};
    ///
    /// let conn = Connection::open_in_memory().unwrap();
    /// User::create_table(&conn).unwrap();
    /// Category::create_table(&conn).unwrap();
    /// let test_user = User::insert("foo@bar.baz".to_string(), "hunter2".to_string(), &conn).unwrap();
    /// let inserted_category = Category::insert("foo".to_string(), test_user.id(), &conn).unwrap();
    ///
    /// let selected_category = Category::select_by_id(inserted_category.id(), &conn).unwrap();
    ///
    /// assert_eq!(inserted_category, selected_category);
    /// ```
    ///
    /// # Errors
    /// Will return an error if:
    /// - `id` does not refer to a valid category,
    /// - or there is some other SQL error.
    pub fn select_by_id(id: DatabaseID, connection: &Connection) -> Result<Category, DbError> {
        connection
            .prepare("SELECT id, name, user_id FROM category WHERE id = :id")
            .map_err(DbError::SqlError)?
            .query_row(&[(":id", &id)], |row| {
                let id: DatabaseID = row.get(0)?;
                let name: String = row.get(1)?;
                let user_id: DatabaseID = row.get(2)?;

                Ok(Category { id, name, user_id })
            })
            .map_err(|e| match e {
                Error::QueryReturnedNoRows => DbError::NotFound,
                e => DbError::SqlError(e),
            })
    }

    /// Retrieve categories in the database for the user `user_id`.
    ///
    /// # Examples
    /// ```
    /// use rusqlite::Connection;
    ///
    /// use backrooms_rs::db::{Model, User, Category};
    ///
    /// let conn = Connection::open_in_memory().unwrap();
    /// User::create_table(&conn).unwrap();
    /// Category::create_table(&conn).unwrap();
    /// let test_user = User::insert("foo@bar.baz".to_string(), "hunter2".to_string(), &conn).unwrap();
    /// let inserted_categories = vec![
    ///     Category::insert("foo".to_string(), test_user.id(), &conn).unwrap(),
    ///     Category::insert("bar".to_string(), test_user.id(), &conn).unwrap()
    /// ];
    ///
    /// let selected_categories = Category::select_by_user_id(test_user.id(), &conn).unwrap();
    ///
    /// assert_eq!(inserted_categories, selected_categories);
    /// ```
    ///
    /// # Errors
    /// Will return an error if:
    /// - `user_id` does not refer to a user id used by a category,
    /// - or there is some other SQL error.
    pub fn select_by_user_id(
        id: DatabaseID,
        connection: &Connection,
    ) -> Result<Vec<Category>, DbError> {
        connection
            .prepare("SELECT id, name, user_id FROM category WHERE user_id = :user_id")
            .map_err(DbError::SqlError)?
            .query_map(&[(":user_id", &id)], |row| {
                let id: DatabaseID = row.get(0)?;
                let name: String = row.get(1)?;
                let user_id: DatabaseID = row.get(2)?;

                Ok(Category { id, name, user_id })
            })
            .map_err(|e| match e {
                Error::QueryReturnedNoRows => DbError::NotFound,
                e => DbError::SqlError(e),
            })?
            .map(|maybe_category| maybe_category.map_err(DbError::SqlError))
            .collect()
    }
}

impl Model for Category {
    fn create_table(connection: &Connection) -> Result<(), DbError> {
        connection
            .execute(
                "CREATE TABLE category (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                user_id INTEGER NOT NULL,
                FOREIGN KEY(user_id) REFERENCES user(id) ON UPDATE CASCADE ON DELETE CASCADE
                )",
                (),
            )
            .map_err(DbError::SqlError)?;

        Ok(())
    }
}

/// An expense or income, i.e. an event where money was either spent or earned.
///
/// New instances should be created through `Transaction::insert(...)`.
pub struct Transaction {
    id: DatabaseID,
    amount: f64,
    date: NaiveDate,
    description: String,
    category_id: DatabaseID,
    user_id: DatabaseID,
}

impl Transaction {
    pub fn id(&self) -> DatabaseID {
        self.id
    }

    pub fn amount(&self) -> f64 {
        self.amount
    }

    pub fn date(&self) -> &NaiveDate {
        &self.date
    }

    pub fn description(&self) -> &str {
        &self.description
    }

    pub fn category_id(&self) -> DatabaseID {
        self.category_id
    }

    pub fn user_id(&self) -> DatabaseID {
        self.user_id
    }

    /// Create a new transaction in the database.
    ///
    /// # Examples
    /// ```
    /// use chrono::NaiveDate;
    /// use rusqlite::Connection;
    ///
    /// use backrooms_rs::db::{Category, Model, Transaction, User};
    ///
    /// let conn = Connection::open_in_memory().unwrap();
    /// User::create_table(&conn).unwrap();
    /// Category::create_table(&conn).unwrap();
    /// Transaction::create_table(&conn).unwrap();
    ///
    /// let user = User::insert("foo@bar.baz".to_string(), "hunter2".to_string(), &conn).unwrap();
    /// let category = Category::insert("Food".to_string(), user.id(), &conn).unwrap();
    ///
    /// let transaction = Transaction::insert(
    ///     3.14,
    ///     NaiveDate::from_ymd_opt(2024, 8, 7).unwrap(),
    ///     "Rust Pie".to_string(),
    ///     category.id(),
    ///     user.id(),
    ///     &conn
    /// )
    /// .unwrap();
    ///
    /// assert_eq!(transaction.amount(), 3.14);
    /// assert_eq!(*transaction.date(), NaiveDate::from_ymd_opt(2024, 8, 7).unwrap());
    /// assert_eq!(transaction.description(), "Rust Pie");
    /// assert_eq!(transaction.category_id(), category.id());
    /// assert_eq!(transaction.user_id(), user.id());
    /// ```
    ///
    /// # Errors
    /// Will return an error if:
    /// - `name` is empty,
    /// - `user_id` does not refer to a valid user,
    /// - or there is some other SQL error.
    pub fn insert(
        amount: f64,
        date: NaiveDate,
        description: String,
        category_id: DatabaseID,
        user_id: DatabaseID,
        connection: &Connection,
    ) -> Result<Transaction, DbError> {
        connection
            .execute(
                "INSERT INTO \"transaction\" (amount, date, description, category_id, user_id) VALUES (?1, ?2, ?3, ?4, ?5)",
                (amount, &date, &description, category_id, user_id),
            )
            .map_err(|e| match e {
                Error::SqliteFailure(error, Some(_)) if error.extended_code == 787 => {
                    DbError::InvalidForeignKey("category_id or user_id".to_string())
                }
                _ => DbError::SqlError(e),
            })?;

        let transaction_id = connection.last_insert_rowid();

        Ok(Transaction {
            id: transaction_id,
            amount,
            date,
            description,
            category_id,
            user_id,
        })
    }
}

impl Model for Transaction {
    fn create_table(connection: &Connection) -> Result<(), DbError> {
        connection
                .execute(
                    "CREATE TABLE \"transaction\" (
                            id INTEGER PRIMARY KEY,
                            amount REAL NOT NULL,
                            date TEXT NOT NULL,
                            description TEXT NOT NULL,
                            category_id INTEGER,
                            user_id INTEGER NOT NULL,
                            FOREIGN KEY(category_id) REFERENCES category(id) ON UPDATE CASCADE ON DELETE CASCADE,
                            FOREIGN KEY(user_id) REFERENCES user(id) ON UPDATE CASCADE ON DELETE CASCADE
                            )",
                    (),
                )
                .map_err(DbError::SqlError)?;

        Ok(())
    }
}

struct SavingsRatio {
    transaction_id: DatabaseID,
    ratio: f64,
}

impl Model for SavingsRatio {
    fn create_table(connection: &Connection) -> Result<(), DbError> {
        connection
            .execute(
                "CREATE TABLE savings_ratio (
                        transaction_id INTEGER PRIMARY KEY,
                        ratio REAL NOT NULL,
                        FOREIGN KEY(transaction_id) REFERENCES \"transaction\"(id) ON UPDATE CASCADE ON DELETE CASCADE
                        )",
                (),
            )
            .map_err(DbError::SqlError)?;

        Ok(())
    }
}

struct RecurringTransaction {
    transaction_id: DatabaseID,
    start_date: NaiveDate,
    end_date: NaiveDate,
    frequency: i64,
}

impl Model for RecurringTransaction {
    fn create_table(connection: &Connection) -> Result<(), DbError> {
        connection
                .execute(
                    "CREATE TABLE recurring_transaction (
                            transaction_id INTEGER PRIMARY KEY,
                            start_date TEXT NOT NULL,
                            end_date TEXT NOT NULL,
                            frequency INTEGER NOT NULL,
                            FOREIGN KEY(transaction_id) REFERENCES \"transaction\"(id) ON UPDATE CASCADE ON DELETE CASCADE
                            )",
                    (),
                )
                .map_err(DbError::SqlError)?;

        Ok(())
    }
}

pub fn initialize(connection: &Connection) -> Result<(), DbError> {
    let transaction =
        SqlTransaction::new_unchecked(connection, rusqlite::TransactionBehavior::Exclusive)
            .map_err(DbError::SqlError)?;

    User::create_table(&transaction)?;
    Category::create_table(&transaction)?;
    Transaction::create_table(&transaction)?;
    SavingsRatio::create_table(&transaction)?;
    RecurringTransaction::create_table(&transaction)?;

    transaction.commit().map_err(DbError::SqlError)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::f64::consts::PI;

    use chrono::NaiveDate;
    use rusqlite::Connection;

    use crate::db::{initialize, Category, DbError, Transaction, User};

    fn init_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();
        conn
    }

    #[test]
    fn create_user() {
        let conn = init_db();

        let email = "hello@world.com";
        let password = "hunter2";

        let inserted_user = User::insert(email.to_string(), password.to_string(), &conn).unwrap();

        assert!(inserted_user.id > 0);
        assert_eq!(inserted_user.email, email);
        assert_eq!(inserted_user.password, password);
    }

    #[test]
    fn create_user_empty_email() {
        let conn = init_db();

        assert_eq!(
            User::insert("".to_string(), "hunter2".to_string(), &conn),
            Err(DbError::EmptyEmail)
        );
    }

    #[test]
    fn create_user_empty_password() {
        let conn = init_db();

        assert_eq!(
            User::insert("foo@bar.baz".to_string(), "".to_string(), &conn),
            Err(DbError::EmptyPassword)
        );
    }

    #[test]
    fn create_user_duplicate_email() {
        let conn = init_db();

        let email = "hello@world.com".to_string();
        let password = "hunter2".to_string();

        assert!(User::insert(email.clone(), password.clone(), &conn).is_ok());
        assert_eq!(
            User::insert(email.clone(), "hunter3".to_string(), &conn),
            Err(DbError::DuplicateEmail)
        );
    }

    #[test]
    fn create_user_duplicate_password() {
        let conn = init_db();

        let email = "hello@world.com".to_string();
        let password = "hunter2".to_string();

        assert!(User::insert(email, password.clone(), &conn).is_ok());
        assert_eq!(
            User::insert("bye@world.com".to_string(), password.clone(), &conn),
            Err(DbError::DuplicatePassword)
        );
    }

    #[test]
    fn select_user_by_non_existent_email() {
        let conn = init_db();
        User::insert("foo@bar.baz".to_string(), "hunter2".to_string(), &conn).unwrap();

        let email = "notavalidemail";

        assert_eq!(User::select(email, &conn), Err(DbError::NotFound));
    }

    #[test]
    fn select_user_by_existing_email() {
        let conn = init_db();

        let test_user =
            User::insert("foo@bar.baz".to_string(), "hunter2".to_string(), &conn).unwrap();
        let retrieved_user = User::select(test_user.email(), &conn).unwrap();

        assert_eq!(retrieved_user, test_user);
    }

    #[test]
    fn create_category() {
        let conn = init_db();
        let test_user =
            User::insert("foo@bar.baz".to_string(), "hunter2".to_string(), &conn).unwrap();

        let name = "Categorically a category";
        let category = Category::insert(name.to_string(), test_user.id(), &conn).unwrap();

        assert!(category.id > 0);
        assert_eq!(category.name, name);
        assert_eq!(category.user_id, test_user.id());
    }

    #[test]
    fn create_category_with_empty_name_returns_error() {
        let conn = init_db();
        let test_user =
            User::insert("foo@bar.baz".to_string(), "hunter2".to_string(), &conn).unwrap();

        let maybe_category = Category::insert("".to_string(), test_user.id(), &conn);

        assert!(matches!(maybe_category, Err(DbError::EmptyField(_))));
    }

    #[test]
    fn create_category_with_invalid_user_id_returns_error() {
        let conn = init_db();

        let maybe_category = Category::insert("Foo".to_string(), 42, &conn);

        assert!(matches!(maybe_category, Err(DbError::InvalidForeignKey(_))));
    }

    #[test]
    fn select_category() {
        let conn = init_db();
        let test_user =
            User::insert("foo@bar.baz".to_string(), "hunter2".to_string(), &conn).unwrap();
        let inserted_category = Category::insert("Foo".to_string(), test_user.id(), &conn).unwrap();

        let selected_category = Category::select_by_id(inserted_category.id(), &conn).unwrap();

        assert_eq!(inserted_category, selected_category);
    }

    #[test]
    fn select_category_with_invalid_id() {
        let conn = init_db();

        let selected_category = Category::select_by_id(1337, &conn);

        assert_eq!(selected_category, Err(DbError::NotFound));
    }

    #[test]
    fn select_category_with_user_id() {
        let conn = init_db();
        let test_user =
            User::insert("foo@bar.baz".to_string(), "hunter2".to_string(), &conn).unwrap();
        let inserted_categories = vec![
            Category::insert("Foo".to_string(), test_user.id(), &conn).unwrap(),
            Category::insert("Bar".to_string(), test_user.id(), &conn).unwrap(),
        ];

        let selected_categories = Category::select_by_user_id(test_user.id(), &conn).unwrap();

        assert_eq!(inserted_categories, selected_categories);
    }

    #[test]
    fn select_category_with_invalid_user_id() {
        let conn = init_db();
        let test_user =
            User::insert("foo@bar.baz".to_string(), "hunter2".to_string(), &conn).unwrap();
        Category::insert("Foo".to_string(), test_user.id(), &conn).unwrap();
        Category::insert("Bar".to_string(), test_user.id(), &conn).unwrap();

        let selected_categories = Category::select_by_user_id(test_user.id() + 1, &conn).unwrap();

        assert_eq!(selected_categories, []);
    }

    #[test]
    fn create_transaction() {
        let conn = init_db();

        let user = User::insert("foo@bar.baz".to_string(), "hunter2".to_string(), &conn).unwrap();
        let category = Category::insert("Food".to_string(), user.id(), &conn).unwrap();

        let amount = PI;
        let date = NaiveDate::from_ymd_opt(2024, 8, 7).unwrap();
        let description = "Rust Pie".to_string();

        let transaction = Transaction::insert(
            amount,
            date,
            description.clone(),
            category.id(),
            user.id(),
            &conn,
        )
        .unwrap();

        assert_eq!(transaction.amount(), amount);
        assert_eq!(*transaction.date(), date);
        assert_eq!(transaction.description(), description);
        assert_eq!(transaction.category_id(), category.id());
        assert_eq!(transaction.user_id(), user.id());
    }
}
