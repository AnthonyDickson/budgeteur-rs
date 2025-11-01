use rusqlite::Connection;
use time::Date;

use crate::{Error, database_id::DatabaseId};

/// The amount of money available for a bank account or credit card.
#[derive(Debug, Clone, PartialEq)]
pub struct Balance {
    /// The id for the account balance.
    pub id: DatabaseId,
    /// The account with which to associate the balance.
    pub account: String,
    /// The balance.
    pub balance: f64,
    /// When the balance was updated.
    pub date: Date,
}

pub fn create_balance_table(connection: &rusqlite::Connection) -> Result<(), rusqlite::Error> {
    connection.execute(
        "CREATE TABLE IF NOT EXISTS balance (
            id INTEGER PRIMARY KEY,
            account TEXT NOT NULL UNIQUE,
            balance REAL NOT NULL,
            date TEXT NOT NULL
        )",
        (),
    )?;

    Ok(())
}

pub fn map_row_to_balance(row: &rusqlite::Row) -> Result<Balance, rusqlite::Error> {
    let id = row.get(0)?;
    let account = row.get(1)?;
    let balance = row.get(2)?;
    let date = row.get(3)?;

    Ok(Balance {
        id,
        account,
        balance,
        date,
    })
}

/// Get the total balance across all accounts.
///
/// # Arguments
/// * `connection` - Database connection reference
///
/// # Errors
/// Returns [Error] if:
/// - Database connection fails
/// - SQL query preparation or execution fails
pub fn get_total_account_balance(connection: &Connection) -> Result<f64, Error> {
    let mut stmt = connection.prepare("SELECT COALESCE(SUM(balance), 0) FROM balance")?;

    let total: f64 = stmt.query_row([], |row| row.get(0))?;

    Ok(total)
}

#[cfg(test)]
mod create_balances_table_tests {
    use rusqlite::Connection;

    use super::create_balance_table;

    #[test]
    fn sql_is_valid() {
        let connection =
            Connection::open_in_memory().expect("Could not initialise in-memory SQLite database");

        assert_eq!(Ok(()), create_balance_table(&connection));
    }
}

#[cfg(test)]
mod get_total_account_balance_tests {
    use rusqlite::Connection;
    use time::macros::date;

    use super::{create_balance_table, get_total_account_balance};

    fn get_test_connection() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        create_balance_table(&conn).unwrap();
        conn
    }

    #[test]
    fn returns_sum_of_all_balances() {
        let conn = get_test_connection();

        // Insert test balances
        conn.execute(
            "INSERT INTO balance (id, account, balance, date) VALUES (?1, ?2, ?3, ?4)",
            (1, "Account 1", 100.50, date!(2024 - 01 - 01).to_string()),
        )
        .unwrap();

        conn.execute(
            "INSERT INTO balance (id, account, balance, date) VALUES (?1, ?2, ?3, ?4)",
            (2, "Account 2", 250.75, date!(2024 - 01 - 01).to_string()),
        )
        .unwrap();

        conn.execute(
            "INSERT INTO balance (id, account, balance, date) VALUES (?1, ?2, ?3, ?4)",
            (3, "Account 3", -50.25, date!(2024 - 01 - 01).to_string()),
        )
        .unwrap();

        let result = get_total_account_balance(&conn).unwrap();

        assert_eq!(result, 301.0);
    }

    #[test]
    fn returns_zero_for_no_balances() {
        let conn = get_test_connection();

        let result = get_total_account_balance(&conn).unwrap();

        assert_eq!(result, 0.0);
    }

    #[test]
    fn handles_negative_balances() {
        let conn = get_test_connection();

        // Insert test balances with negative total
        conn.execute(
            "INSERT INTO balance (id, account, balance, date) VALUES (?1, ?2, ?3, ?4)",
            (1, "Account 1", -200.0, date!(2024 - 01 - 01).to_string()),
        )
        .unwrap();

        conn.execute(
            "INSERT INTO balance (id, account, balance, date) VALUES (?1, ?2, ?3, ?4)",
            (2, "Account 2", 100.0, date!(2024 - 01 - 01).to_string()),
        )
        .unwrap();

        let result = get_total_account_balance(&conn).unwrap();

        assert_eq!(result, -100.0);
    }
}
