use chrono::{NaiveDate, Utc};
use common::DatabaseID;
use rusqlite::{
    types::{FromSql, FromSqlError, FromSqlResult, ToSql, ToSqlOutput},
    Connection, Error, Row, Transaction as SqlTransaction,
};
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
    /// A query was given an invalid foreign key. The client should check that the ids are valid.
    InvalidForeignKey,
    /// An invalid date was provided (e.g., a future date on a transaction or an end date before or on a start date for recurring transactions).
    /// The client should try again with a date no later than today.
    InvalidDate,
    /// An invalid ratio was given. The client should try again with a number between 0.0 and 1.0 (inclusive).
    InvalidRatio,
    /// The row could not be found with the provided info (e.g., id). The client should try again with different parameters.
    NotFound,
    /// Wrapper for Sqlite errors not handled by the other enum entries.
    SqlError(Error),
}

impl From<Error> for DbError {
    fn from(error: Error) -> Self {
        match error {
            // Code 787 occurs when a FOREIGN KEY constraint failed.
            Error::SqliteFailure(error, Some(_)) if error.extended_code == 787 => {
                DbError::InvalidForeignKey
            }
            // Code 2067 occurs when a UNIQUE constraint failed.
            Error::SqliteFailure(sql_error, Some(ref desc))
                if sql_error.extended_code == 2067 && desc.contains("email") =>
            {
                DbError::DuplicateEmail
            }
            Error::SqliteFailure(sql_error, Some(ref desc))
                if sql_error.extended_code == 2067 && desc.contains("password") =>
            {
                DbError::DuplicatePassword
            }
            Error::QueryReturnedNoRows => DbError::NotFound,
            e => DbError::SqlError(e),
        }
    }
}

pub trait Model<T> {
    /// Create a table for the model.
    ///
    /// # Errors
    /// Returns an error if the table already exists or if there is an SQL error.
    fn create_table(connection: &Connection) -> Result<(), DbError>;

    /// Convert a row into a concrete type.
    ///
    /// **Note:** This function expects that the row object contains all the table columns in the order they were defined.
    ///
    /// # Errors
    /// Returns an error if a row item cannot be converted into the corresponding rust type, or if an invalid column index was used.
    fn map_row(row: &Row) -> Result<T, Error> {
        Self::map_row_with_offset(row, 0)
    }

    /// Convert a row into a concrete type.
    ///
    /// The `offset` indicates which column the row should be read from.
    /// This is useful in cases where tables have been joined and you want to construct two different types from the one query.
    ///
    /// **Note:** This function expects that the row object contains all the table columns in the order they were defined.
    ///
    /// # Examples
    /// ```
    /// use rusqlite::{Connection, Error, Row};
    ///
    /// use backend::db::{DbError, Model};
    ///
    /// struct Foo {
    ///     id: i64,
    ///     desc: String
    /// }
    ///
    /// impl Model<Foo> for Foo {
    ///     fn create_table(connection: &Connection) -> Result<(), DbError> {
    ///         connection.execute(
    ///             "CREATE TABLE bar (id INTEGER PRIMARY KEY, desc TEXT NOT NULL)",
    ///             (),
    ///         )?;
    ///
    ///         Ok(())
    ///     }
    ///
    ///     fn map_row_with_offset(row: &Row, offset: usize) -> Result<Foo, Error> {
    ///         Ok(Foo {
    ///             id: row.get(offset)?,
    ///             desc: row.get(offset + 1)?,
    ///         })
    ///     }
    /// }
    ///
    /// struct Bar {
    ///     id: i64,
    ///     desc: String
    /// }
    ///
    /// impl Model<Bar> for Bar {
    ///     fn create_table(connection: &Connection) -> Result<(), DbError> {
    ///         connection.execute(
    ///             "CREATE TABLE bar (id INTEGER PRIMARY KEY, desc TEXT NOT NULL)",
    ///             (),
    ///         )?;
    ///
    ///         Ok(())
    ///     }
    ///
    ///     fn map_row_with_offset(row: &Row, offset: usize) -> Result<Bar, Error> {
    ///         Ok(Bar {
    ///             id: row.get(offset)?,
    ///             desc: row.get(offset + 1)?,
    ///         })
    ///     }
    /// }
    ///
    /// fn example(conn: &Connection) -> Result<(Foo, Bar), DbError> {
    ///     conn.
    ///         prepare("SELECT l.id, l.desc, r.id, r.desc FROM foo l INNER JOIN bar r ON l.id = r.foo_id WHERE l.id = :id")?
    ///         .query_row(&[(":id", &1)], |row| {
    ///             let foo = Foo::map_row(row)?;
    ///             let bar = Bar::map_row_with_offset(row, 2)?;
    ///
    ///             Ok((foo, bar))
    ///         })
    ///         .map_err(|e| e.into())
    /// }
    /// ```
    ///
    /// # Errors
    /// Returns an error if a row item cannot be converted into the corresponding rust type, or if an invalid column index was used.
    fn map_row_with_offset(row: &Row, offset: usize) -> Result<T, Error>;
}

/// A user of the application.
///
/// New instances should be created through `User::insert(...)`.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct User {
    id: DatabaseID,
    email: String,
    password: String,
}

impl Model<User> for User {
    fn create_table(connection: &Connection) -> Result<(), DbError> {
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

    fn map_row_with_offset(row: &Row, offset: usize) -> Result<User, Error> {
        Ok(User {
            id: row.get(offset)?,
            email: row.get(offset + 1)?,
            password: row.get(offset + 2)?,
        })
    }
}

// TODO: Merge db::User into common::User. Move database specific items to a trait and implement that trait here. Try to create generic trait for insert and select operations (generic arguments and return type).

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
    /// This function will return an error if there was a problem executing the SQL query. This could be due to:
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

        connection.execute(
            "INSERT INTO user (email, password) VALUES (?1, ?2)",
            (&email, &password_hash),
        )?;

        let id = connection.last_insert_rowid();

        Ok(User {
            id,
            email,
            password: password_hash,
        })
    }

    /// Get the user from the database that has the specified `email` address or `None` if such user does not exist.
    ///
    /// # Examples
    /// ```
    /// use rusqlite::Connection;
    ///
    /// use backend::db::{Model, User};
    ///
    /// let conn = Connection::open_in_memory().unwrap();
    /// User::create_table(&conn).unwrap();
    /// let inserted_user = User::insert("foo@bar.baz".to_string(), "hunter2".to_string(), &conn).unwrap();
    ///
    /// let selected_user = User::select_by_email(inserted_user.email(), &conn).unwrap();
    ///
    /// assert_eq!(inserted_user, selected_user);
    /// ```
    /// # Panics
    ///
    /// Panics if there are SQL related errors.
    pub fn select_by_email(email: &str, connection: &Connection) -> Result<User, DbError> {
        connection
            .prepare("SELECT id, email, password FROM user WHERE email = :email")?
            .query_row(&[(":email", &email)], User::map_row)
            .map_err(|e| e.into())
    }
}

/// A category for expenses and income, e.g., 'Groceries', 'Eating Out', 'Wages'.
///
/// New instances should be created through `Category::insert(...)`.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Category {
    id: DatabaseID,
    name: String,
    user_id: DatabaseID,
}

impl Model<Category> for Category {
    fn create_table(connection: &Connection) -> Result<(), DbError> {
        connection.execute(
            "CREATE TABLE category (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                user_id INTEGER NOT NULL,
                FOREIGN KEY(user_id) REFERENCES user(id) ON UPDATE CASCADE ON DELETE CASCADE
                )",
            (),
        )?;

        Ok(())
    }

    fn map_row_with_offset(row: &Row, offset: usize) -> Result<Category, Error> {
        Ok(Category {
            id: row.get(offset)?,
            name: row.get(offset + 1)?,
            user_id: row.get(offset + 2)?,
        })
    }
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
    /// use backend::db::{Model, User, Category};
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
    /// This function will return an error if:
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

        connection.execute(
            "INSERT INTO category (name, user_id) VALUES (?1, ?2)",
            (&name, user_id),
        )?;

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
    /// use backend::db::{Model, User, Category};
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
    /// This function will return an error if:
    /// - `id` does not refer to a valid category,
    /// - or there is some other SQL error.
    pub fn select_by_id(id: DatabaseID, connection: &Connection) -> Result<Category, DbError> {
        let category = connection
            .prepare("SELECT id, name, user_id FROM category WHERE id = :id")?
            .query_row(&[(":id", &id)], Category::map_row)?;

        Ok(category)
    }

    /// Retrieve categories in the database for the user `user_id`.
    ///
    /// # Examples
    /// ```
    /// use rusqlite::Connection;
    ///
    /// use backend::db::{Model, User, Category};
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
    /// This function will return an error if there is an SQL error.
    pub fn select_by_user_id(
        user_id: DatabaseID,
        connection: &Connection,
    ) -> Result<Vec<Category>, DbError> {
        connection
            .prepare("SELECT id, name, user_id FROM category WHERE user_id = :user_id")?
            .query_map(&[(":user_id", &user_id)], Category::map_row)?
            .map(|maybe_category| maybe_category.map_err(DbError::SqlError))
            .collect()
    }
}

/// An expense or income, i.e. an event where money was either spent or earned.
///
/// New instances should be created through `Transaction::insert(...)`.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Transaction {
    id: DatabaseID,
    amount: f64,
    date: NaiveDate,
    description: String,
    category_id: DatabaseID,
    user_id: DatabaseID,
}

impl Model<Transaction> for Transaction {
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
                )?;

        Ok(())
    }

    fn map_row_with_offset(row: &Row, offset: usize) -> Result<Transaction, Error> {
        Ok(Transaction {
            id: row.get(offset)?,
            amount: row.get(offset + 1)?,
            date: row.get(offset + 2)?,
            description: row.get(offset + 3)?,
            category_id: row.get(offset + 4)?,
            user_id: row.get(offset + 5)?,
        })
    }
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
    /// Dates must be no later than today.
    ///
    /// # Examples
    /// ```
    /// use chrono::NaiveDate;
    /// use rusqlite::Connection;
    ///
    /// use backend::db::{Category, Model, Transaction, User};
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
    /// This function will return an error if:
    /// - `date` refers to a future date,
    /// - `name` is empty,
    /// - `category_id` does not refer to a valid category,
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
        if date > Utc::now().date_naive() {
            return Err(DbError::InvalidDate);
        }

        // TODO: Ensure that the category id refers to a category owned by the user.
        connection
            .execute(
                "INSERT INTO \"transaction\" (amount, date, description, category_id, user_id) VALUES (?1, ?2, ?3, ?4, ?5)",
                (amount, &date, &description, category_id, user_id),
            )?;

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

    /// Retrieve a transaction in the database by its `id`.
    ///
    /// # Examples
    /// ```
    /// use chrono::NaiveDate;
    /// use rusqlite::Connection;
    ///
    /// use backend::db::{Model, User, Category, Transaction};
    ///
    /// let conn = Connection::open_in_memory().unwrap();
    /// User::create_table(&conn).unwrap();
    /// Category::create_table(&conn).unwrap();
    /// Transaction::create_table(&conn).unwrap();
    /// let user = User::insert("foo@bar.baz".to_string(), "hunter2".to_string(), &conn).unwrap();
    /// let category = Category::insert("foo".to_string(), user.id(), &conn).unwrap();
    ///
    /// let inserted_transaction = Transaction::insert(
    ///     3.14,
    ///     NaiveDate::from_ymd_opt(2024, 8, 7).unwrap(),
    ///     "Rust Pie".to_string(),
    ///     category.id(),
    ///     user.id(),
    ///     &conn
    /// )
    /// .unwrap();
    ///
    /// let selected_transaction = Transaction::select_by_id(inserted_transaction.id(), &conn).unwrap();
    ///
    /// assert_eq!(inserted_transaction, selected_transaction);
    /// ```
    ///
    /// # Errors
    /// This function will return an error if:
    /// - `id` does not refer to a valid transaction,
    /// - or there is some other SQL error.
    pub fn select_by_id(id: DatabaseID, connection: &Connection) -> Result<Transaction, DbError> {
        let transaction = connection
            .prepare("SELECT id, amount, date, description, category_id, user_id FROM \"transaction\" WHERE id = :id")?
            .query_row(&[(":id", &id)], Transaction::map_row)?;

        Ok(transaction)
    }

    /// Retrieve the transactions in the database for the user `user_id`.
    ///
    /// # Examples
    /// ```
    /// use backend::db::{User, Transaction};
    ///
    /// fn sum_transaction_amount_for_user(user: &User, conn: &rusqlite::Connection) -> f64 {
    ///     let transactions = Transaction::select_by_user_id(user.id(), conn).unwrap();
    ///     transactions.iter().map(|transaction| transaction.amount()).sum()
    /// }
    /// ```
    ///
    /// # Errors
    /// This function will return an error if there is an SQL error.
    pub fn select_by_user_id(
        user_id: DatabaseID,
        connection: &Connection,
    ) -> Result<Vec<Transaction>, DbError> {
        connection
            .prepare("SELECT id, amount, date, description, category_id, user_id FROM \"transaction\" WHERE user_id = :user_id")?
            .query_map(&[(":user_id", &user_id)], Transaction::map_row)?
            .map(|maybe_category| maybe_category.map_err(DbError::SqlError))
            .collect()
    }
}

/// The amount of an income transaction that should counted as savings.
///
/// This object must be attached to an existing transaction and cannot exist independently.
///
/// New instances should be created through `SavingsRatio::insert(...)`.
#[derive(Debug, PartialEq)]
pub struct SavingsRatio {
    transaction_id: DatabaseID,
    ratio: f64,
}

impl Model<SavingsRatio> for SavingsRatio {
    fn create_table(connection: &Connection) -> Result<(), DbError> {
        connection
            .execute(
                "CREATE TABLE savings_ratio (
                        transaction_id INTEGER PRIMARY KEY,
                        ratio REAL NOT NULL,
                        FOREIGN KEY(transaction_id) REFERENCES \"transaction\"(id) ON UPDATE CASCADE ON DELETE CASCADE
                        )",
                (),
            )?;

        Ok(())
    }

    fn map_row_with_offset(row: &Row, offset: usize) -> Result<SavingsRatio, Error> {
        Ok(SavingsRatio {
            transaction_id: row.get(offset)?,
            ratio: row.get(offset + 1)?,
        })
    }
}

impl SavingsRatio {
    pub fn transaction_id(&self) -> DatabaseID {
        self.transaction_id
    }

    pub fn ratio(&self) -> f64 {
        self.ratio
    }

    /// Create a new savings ratio in the database.
    ///
    /// # Examples
    /// ```
    /// use rusqlite::Connection;
    ///
    /// use backend::db::{Transaction, SavingsRatio};
    ///
    /// fn set_savings_ratio(transaction: &Transaction, ratio: f64, connection: &Connection) -> SavingsRatio {
    ///     SavingsRatio::insert(transaction.id(), ratio, connection).unwrap()
    /// }
    /// ```
    ///
    /// # Errors
    /// This function will return an error if:
    /// - `transaction_id` does not refer to a valid transaction,
    /// - `ratio` is not a ratio between zero and one (inclusive),
    /// - or there is some other SQL error.
    pub fn insert(
        transaction_id: DatabaseID,
        ratio: f64,
        connection: &Connection,
    ) -> Result<SavingsRatio, DbError> {
        if !(0.0..=1.0).contains(&ratio) || ratio.is_sign_negative() {
            return Err(DbError::InvalidRatio);
        }

        connection.execute(
            "INSERT INTO savings_ratio (transaction_id, ratio) VALUES (?1, ?2)",
            (transaction_id, ratio),
        )?;

        Ok(SavingsRatio {
            transaction_id,
            ratio,
        })
    }
}

/// How often a recurring transaction happens.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Frequency {
    Daily,
    Weekly,
    Fortnightly,
    /// A calendar month of variable length.
    Monthly,
    /// A calendar quarter (Jan-Mar, Apr-Jun, Jul-Sep, Oct-Dec).
    Quarterly,
    Yearly,
}

impl ToSql for Frequency {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::Owned(rusqlite::types::Value::Integer(
            match self {
                Frequency::Daily => 0,
                Frequency::Weekly => 1,
                Frequency::Fortnightly => 2,
                Frequency::Monthly => 3,
                Frequency::Quarterly => 4,
                Frequency::Yearly => 5,
            },
        )))
    }
}

impl FromSql for Frequency {
    fn column_result(value: rusqlite::types::ValueRef<'_>) -> FromSqlResult<Self> {
        value.as_i64().and_then(|number| match number {
            0 => Ok(Frequency::Daily),
            1 => Ok(Frequency::Weekly),
            2 => Ok(Frequency::Fortnightly),
            3 => Ok(Frequency::Monthly),
            4 => Ok(Frequency::Quarterly),
            5 => Ok(Frequency::Yearly),
            _ => Err(FromSqlError::OutOfRange(number)),
        })
    }
}

/// A transaction (income or expense) that repeats on a regular basis (e.g., wages, phone bill).
///
/// This object must be attached to an existing transaction and cannot exist independently.
///
/// New instances should be created through `RecurringTransaction::insert(...)`.
#[derive(Debug, PartialEq)]
pub struct RecurringTransaction {
    transaction_id: DatabaseID,
    end_date: Option<NaiveDate>,
    frequency: Frequency,
}

impl Model<RecurringTransaction> for RecurringTransaction {
    fn create_table(connection: &Connection) -> Result<(), DbError> {
        connection
            .execute(
                "CREATE TABLE recurring_transaction (
                        transaction_id INTEGER PRIMARY KEY,
                        end_date TEXT NOT NULL,
                        frequency INTEGER NOT NULL,
                        FOREIGN KEY(transaction_id) REFERENCES \"transaction\"(id) ON UPDATE CASCADE ON DELETE CASCADE
                        )",
                (),
            )?;

        Ok(())
    }

    fn map_row_with_offset(row: &Row, offset: usize) -> Result<RecurringTransaction, Error> {
        Ok(RecurringTransaction {
            transaction_id: row.get(offset)?,
            end_date: row.get(offset + 1)?,
            frequency: row.get(offset + 2)?,
        })
    }
}

impl RecurringTransaction {
    pub fn transaction_id(&self) -> DatabaseID {
        self.transaction_id
    }

    pub fn end_date(&self) -> &Option<NaiveDate> {
        &self.end_date
    }

    pub fn frequency(&self) -> Frequency {
        self.frequency
    }

    /// Create a new recurring transaction in the database.
    ///
    /// This will attach to an existing transaction and convert that transaction from a once-off transaction to a recurring one.
    ///
    /// # Examples
    /// ```
    /// use chrono::Utc;
    /// use rusqlite::Connection;
    ///
    /// use backend::db::{Frequency, Transaction, RecurringTransaction};
    ///
    /// fn set_recurring(
    ///     transaction: &Transaction,
    ///     frequency: Frequency,
    ///     connection: &Connection
    /// ) -> RecurringTransaction {
    ///     RecurringTransaction::insert(
    ///         &transaction,
    ///         None,
    ///         frequency,
    ///         connection
    ///     )
    ///     .unwrap()
    /// }
    /// ```
    ///
    /// # Errors
    /// This function will return an error if:
    /// - `end_date` is on or before `transaction.date()`,
    /// - or there is some other SQL error.
    pub fn insert(
        transaction: &Transaction,
        end_date: Option<NaiveDate>,
        frequency: Frequency,
        connection: &Connection,
    ) -> Result<RecurringTransaction, DbError> {
        if end_date.is_some_and(|date| date <= *transaction.date()) {
            return Err(DbError::InvalidDate);
        }

        connection.execute(
            "INSERT INTO recurring_transaction (transaction_id, end_date, frequency) VALUES (?1, ?2, ?3)",
            (transaction.id(), end_date, frequency),
        )?;

        Ok(RecurringTransaction {
            transaction_id: transaction.id(),
            end_date,
            frequency,
        })
    }
}

pub fn initialize(connection: &Connection) -> Result<(), DbError> {
    let transaction =
        SqlTransaction::new_unchecked(connection, rusqlite::TransactionBehavior::Exclusive)?;

    User::create_table(&transaction)?;
    Category::create_table(&transaction)?;
    Transaction::create_table(&transaction)?;
    SavingsRatio::create_table(&transaction)?;
    RecurringTransaction::create_table(&transaction)?;

    transaction.commit()?;

    Ok(())
}

// TODO: Separate types for recurring schedule (current RecurringTransaction type) and join result row of transaction and recurring schedule?
pub fn select_recurring_transactions_by_user(
    user: &User,
    connection: &Connection,
) -> Result<Vec<(Transaction, RecurringTransaction)>, DbError> {
    connection
        .prepare(
            "SELECT l.id, l.amount, l.date, l.description, l.category_id, l.user_id, r.transaction_id, r.end_date, r.frequency
            FROM \"transaction\" l
            INNER JOIN recurring_transaction r ON r.transaction_id = l.id
            WHERE l.user_id = :user_id;",
        )?
        .query_and_then(&[(":user_id", &user.id())], |row| {
            let transaction = Transaction::map_row(row)?;
            let recurring_transaction = RecurringTransaction::map_row_with_offset(row, 6)?;

            Ok((transaction, recurring_transaction))
        })?
        .map(|item| item.map_err(DbError::SqlError))
        .collect()
}

#[cfg(test)]
mod tests {
    use std::f64::consts::PI;

    use chrono::{Days, Months, NaiveDate, Utc};
    use rusqlite::Connection;

    use crate::db::{initialize, Category, DbError, SavingsRatio, Transaction, User};

    use super::{select_recurring_transactions_by_user, Frequency, RecurringTransaction};

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

        assert_eq!(User::select_by_email(email, &conn), Err(DbError::NotFound));
    }

    #[test]
    fn select_user_by_existing_email() {
        let conn = init_db();

        let test_user =
            User::insert("foo@bar.baz".to_string(), "hunter2".to_string(), &conn).unwrap();
        let retrieved_user = User::select_by_email(test_user.email(), &conn).unwrap();

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

        assert_eq!(maybe_category, Err(DbError::InvalidForeignKey));
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
        let date = Utc::now().date_naive();
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

    #[test]
    fn create_transaction_fails_on_invalid_user_id() {
        let conn = init_db();

        let user = User::insert("foo@bar.baz".to_string(), "hunter2".to_string(), &conn).unwrap();
        let category = Category::insert("Food".to_string(), user.id(), &conn).unwrap();

        let amount = PI;
        let date = Utc::now().date_naive();
        let description = "Rust Pie".to_string();

        let maybe_transaction = Transaction::insert(
            amount,
            date,
            description.clone(),
            category.id(),
            user.id() + 1,
            &conn,
        );

        assert_eq!(maybe_transaction, Err(DbError::InvalidForeignKey));
    }

    #[test]
    fn create_transaction_fails_on_invalid_category_id() {
        let conn = init_db();

        let user = User::insert("foo@bar.baz".to_string(), "hunter2".to_string(), &conn).unwrap();
        let category = Category::insert("Food".to_string(), user.id(), &conn).unwrap();

        let amount = PI;
        let date = Utc::now().date_naive();
        let description = "Rust Pie".to_string();

        let maybe_transaction = Transaction::insert(
            amount,
            date,
            description.clone(),
            category.id() + 1,
            user.id(),
            &conn,
        );

        assert_eq!(maybe_transaction, Err(DbError::InvalidForeignKey));
    }

    #[test]
    fn create_transaction_fails_on_future_date() {
        let conn = init_db();

        let user = User::insert("foo@bar.baz".to_string(), "hunter2".to_string(), &conn).unwrap();
        let category = Category::insert("Food".to_string(), user.id(), &conn).unwrap();

        let amount = PI;
        let date = Utc::now()
            .date_naive()
            .checked_add_days(Days::new(1))
            .unwrap();
        let description = "Rust Pie".to_string();

        let maybe_transaction = Transaction::insert(
            amount,
            date,
            description.clone(),
            category.id(),
            user.id(),
            &conn,
        );

        assert_eq!(maybe_transaction, Err(DbError::InvalidDate));
    }

    #[test]
    fn select_transaction_by_id() {
        let conn = init_db();
        let user = User::insert("foo@bar.baz".to_string(), "hunter2".to_string(), &conn).unwrap();
        let category = Category::insert("foo".to_string(), user.id(), &conn).unwrap();

        let inserted_transaction = Transaction::insert(
            PI,
            NaiveDate::from_ymd_opt(2024, 8, 7).unwrap(),
            "Rust Pie".to_string(),
            category.id(),
            user.id(),
            &conn,
        )
        .unwrap();

        let selected_transaction =
            Transaction::select_by_id(inserted_transaction.id(), &conn).unwrap();

        assert_eq!(inserted_transaction, selected_transaction);
    }

    #[test]
    fn select_transaction_by_invalid_id_fails() {
        let conn = init_db();
        let user = User::insert("foo@bar.baz".to_string(), "hunter2".to_string(), &conn).unwrap();
        let category = Category::insert("foo".to_string(), user.id(), &conn).unwrap();

        let inserted_transaction = Transaction::insert(
            PI,
            NaiveDate::from_ymd_opt(2024, 8, 7).unwrap(),
            "Rust Pie".to_string(),
            category.id(),
            user.id(),
            &conn,
        )
        .unwrap();

        let maybe_transaction = Transaction::select_by_id(inserted_transaction.id() + 1, &conn);

        assert_eq!(maybe_transaction, Err(DbError::NotFound));
    }

    #[test]
    fn select_transactions_by_user_id() {
        let conn = init_db();
        let user = User::insert("foo@bar.baz".to_string(), "hunter2".to_string(), &conn).unwrap();
        let category = Category::insert("foo".to_string(), user.id(), &conn).unwrap();

        let expected_transactions = vec![
            Transaction::insert(
                PI,
                NaiveDate::from_ymd_opt(2024, 8, 7).unwrap(),
                "Rust Pie".to_string(),
                category.id(),
                user.id(),
                &conn,
            )
            .unwrap(),
            Transaction::insert(
                PI + 1.0,
                NaiveDate::from_ymd_opt(2024, 8, 8).unwrap(),
                "Rust Pif".to_string(),
                category.id(),
                user.id(),
                &conn,
            )
            .unwrap(),
        ];

        let transactions = Transaction::select_by_user_id(user.id(), &conn).unwrap();

        assert_eq!(transactions, expected_transactions);
    }

    #[test]
    fn create_savings_ratio() {
        let conn = init_db();
        let user = User::insert("foo@bar.baz".to_string(), "hunter2".to_string(), &conn).unwrap();
        let category = Category::insert("foo".to_string(), user.id(), &conn).unwrap();
        let transaction = Transaction::insert(
            PI,
            NaiveDate::from_ymd_opt(2024, 8, 7).unwrap(),
            "Rust Pie".to_string(),
            category.id(),
            user.id(),
            &conn,
        )
        .unwrap();

        let ratio = 0.5;
        let savings_ratio = SavingsRatio::insert(transaction.id(), ratio, &conn).unwrap();

        assert_eq!(savings_ratio.transaction_id(), transaction.id());
        assert_eq!(savings_ratio.ratio(), ratio);
    }

    #[test]
    fn create_savings_ratio_fails_with_ratio_below_zero() {
        let conn = init_db();
        let user = User::insert("foo@bar.baz".to_string(), "hunter2".to_string(), &conn).unwrap();
        let category = Category::insert("foo".to_string(), user.id(), &conn).unwrap();
        let transaction = Transaction::insert(
            PI,
            NaiveDate::from_ymd_opt(2024, 8, 7).unwrap(),
            "Rust Pie".to_string(),
            category.id(),
            user.id(),
            &conn,
        )
        .unwrap();

        let ratio = -0.01;
        let savings_ratio = SavingsRatio::insert(transaction.id(), ratio, &conn);

        assert_eq!(savings_ratio, Err(DbError::InvalidRatio));
    }

    #[test]
    fn create_savings_ratio_fails_with_negative_zero_ratio() {
        let conn = init_db();
        let user = User::insert("foo@bar.baz".to_string(), "hunter2".to_string(), &conn).unwrap();
        let category = Category::insert("foo".to_string(), user.id(), &conn).unwrap();
        let transaction = Transaction::insert(
            PI,
            NaiveDate::from_ymd_opt(2024, 8, 7).unwrap(),
            "Rust Pie".to_string(),
            category.id(),
            user.id(),
            &conn,
        )
        .unwrap();

        let ratio = -0.0;
        let savings_ratio = SavingsRatio::insert(transaction.id(), ratio, &conn);

        assert_eq!(savings_ratio, Err(DbError::InvalidRatio));
    }

    #[test]
    fn create_savings_ratio_fails_with_ratio_above_one() {
        let conn = init_db();
        let user = User::insert("foo@bar.baz".to_string(), "hunter2".to_string(), &conn).unwrap();
        let category = Category::insert("foo".to_string(), user.id(), &conn).unwrap();
        let transaction = Transaction::insert(
            PI,
            NaiveDate::from_ymd_opt(2024, 8, 7).unwrap(),
            "Rust Pie".to_string(),
            category.id(),
            user.id(),
            &conn,
        )
        .unwrap();

        let ratio = 1.01;
        let savings_ratio = SavingsRatio::insert(transaction.id(), ratio, &conn);

        assert_eq!(savings_ratio, Err(DbError::InvalidRatio));
    }

    #[test]
    fn create_recurring_transaction() {
        let conn = init_db();
        let user = User::insert("foo@bar.baz".to_string(), "hunter2".to_string(), &conn).unwrap();
        let category = Category::insert("foo".to_string(), user.id(), &conn).unwrap();
        let transaction = Transaction::insert(
            PI,
            NaiveDate::from_ymd_opt(2024, 8, 7).unwrap(),
            "Rust Pie".to_string(),
            category.id(),
            user.id(),
            &conn,
        )
        .unwrap();

        let end_date = transaction.date().checked_add_months(Months::new(3));

        let recurring =
            RecurringTransaction::insert(&transaction, end_date, Frequency::Weekly, &conn).unwrap();

        assert_eq!(recurring.transaction_id(), transaction.id);
        assert_eq!(*recurring.end_date(), end_date);
        assert_eq!(recurring.frequency(), Frequency::Weekly);
    }

    #[test]
    fn create_recurring_transaction_fails_on_past_end_date() {
        let conn = init_db();
        let user = User::insert("foo@bar.baz".to_string(), "hunter2".to_string(), &conn).unwrap();
        let category = Category::insert("foo".to_string(), user.id(), &conn).unwrap();
        let transaction = Transaction::insert(
            PI,
            NaiveDate::from_ymd_opt(2024, 8, 7).unwrap(),
            "Rust Pie".to_string(),
            category.id(),
            user.id(),
            &conn,
        )
        .unwrap();

        assert_eq!(
            RecurringTransaction::insert(
                &transaction,
                Some(*transaction.date()),
                Frequency::Weekly,
                &conn
            ),
            Err(DbError::InvalidDate)
        );

        assert_eq!(
            RecurringTransaction::insert(
                &transaction,
                Some(transaction.date().checked_sub_days(Days::new(1)).unwrap()),
                Frequency::Weekly,
                &conn
            ),
            Err(DbError::InvalidDate)
        );
    }

    #[test]
    fn select_recurring_transactions_succeeds() {
        let conn = init_db();
        let user = User::insert("foo@bar.baz".to_string(), "hunter2".to_string(), &conn).unwrap();
        let category = Category::insert("foo".to_string(), user.id(), &conn).unwrap();
        let inserted_transaction = Transaction::insert(
            PI,
            NaiveDate::from_ymd_opt(2024, 8, 7).unwrap(),
            "Rust Pie".to_string(),
            category.id(),
            user.id(),
            &conn,
        )
        .unwrap();

        let end_date = inserted_transaction
            .date()
            .checked_add_months(Months::new(3));

        let inserted_recurring_transction =
            RecurringTransaction::insert(&inserted_transaction, end_date, Frequency::Weekly, &conn)
                .unwrap();

        let expected = vec![(inserted_transaction, inserted_recurring_transction)];

        let results = select_recurring_transactions_by_user(&user, &conn).unwrap();

        assert_eq!(results, expected);
    }

    #[test]
    fn select_recurring_transactions_returns_empty_list() {
        let conn = init_db();
        let user = User::insert("foo@bar.baz".to_string(), "hunter2".to_string(), &conn).unwrap();

        let expected = vec![];
        let results = select_recurring_transactions_by_user(&user, &conn).unwrap();

        assert_eq!(results, expected);
    }
}
