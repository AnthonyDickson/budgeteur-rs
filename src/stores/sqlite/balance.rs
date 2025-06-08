//! Implements a SQLite backed balance store.
use std::sync::{Arc, Mutex};

use time::Date;

use crate::{
    Error,
    db::{CreateTable, MapRow},
    models::Balance,
    stores::BalanceStore,
};

/// Create and retrieve account balances.
#[derive(Debug, Clone)]
pub struct SQLiteBalanceStore {
    connection: Arc<Mutex<rusqlite::Connection>>,
}

impl SQLiteBalanceStore {
    /// Create a new store from the SQLite `connection`.
    pub fn new(connection: Arc<Mutex<rusqlite::Connection>>) -> Self {
        Self { connection }
    }
}

impl CreateTable for SQLiteBalanceStore {
    fn create_table(connection: &rusqlite::Connection) -> Result<(), rusqlite::Error> {
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
}

impl MapRow for SQLiteBalanceStore {
    type ReturnType = Balance;

    fn map_row_with_offset(
        row: &rusqlite::Row,
        offset: usize,
    ) -> Result<Self::ReturnType, rusqlite::Error> {
        let id = row.get(offset)?;
        let account = row.get(offset + 1)?;
        let balance = row.get(offset + 2)?;
        let date = row.get(offset + 3)?;

        Ok(Balance {
            id,
            account,
            balance,
            date,
        })
    }
}

impl BalanceStore for SQLiteBalanceStore {
    fn upsert(&mut self, account: &str, balance: f64, date: &Date) -> Result<Balance, Error> {
        let connection = self
            .connection
            .lock()
            .expect("Could not acquire lock to database");

        let next_id: i64 =
            connection.query_row("SELECT COALESCE(MAX(id), 0) FROM balance;", [], |row| {
                row.get(0)
            })?;
        let next_id = next_id + 1;

        connection.execute(
            "INSERT INTO balance AS b (id, account, balance, date)
                VALUES (?1, ?2, ?3, ?4)
                ON CONFLICT(account) DO UPDATE SET
                    balance=excluded.balance,
                    date=excluded.date
                WHERE excluded.date > b.date AND b.account = excluded.account;",
            (next_id, account, balance, date),
        )?;

        let balance = connection
            .prepare("SELECT id, account, balance, date FROM balance WHERE account = :account;")?
            .query_row(&[(":account", account)], SQLiteBalanceStore::map_row)?;

        Ok(balance)
    }

    fn get_all(&self) -> Result<Vec<Balance>, Error> {
        self.connection
            .lock()
            .expect("Could not acquire database lock")
            .prepare("SELECT id, account, balance, date FROM balance;")?
            .query_map([], SQLiteBalanceStore::map_row)?
            .map(|maybe_balance| maybe_balance.map_err(|error| error.into()))
            .collect()
    }
}

#[cfg(test)]
mod sqlite_balance_store_tests {
    use std::sync::{Arc, Mutex};

    use rusqlite::Connection;
    use time::macros::date;

    use crate::{db::CreateTable, models::Balance, stores::BalanceStore};

    use super::SQLiteBalanceStore;

    fn get_test_store() -> SQLiteBalanceStore {
        let connection = Connection::open_in_memory().unwrap();
        SQLiteBalanceStore::create_table(&connection).unwrap();
        let connection = Arc::new(Mutex::new(connection));

        SQLiteBalanceStore::new(connection.clone())
    }

    #[tokio::test]
    async fn can_upsert_balance() {
        let mut store = get_test_store();
        let want = Balance {
            id: 1,
            account: "1234-5678-9101-012".to_owned(),
            balance: 37_337_252_784.63,
            date: date!(2025 - 05 - 31),
        };

        let got = store
            .upsert(&want.account, want.balance, &want.date)
            .expect("Could not create account balance");

        assert_eq!(want, got, "want balance {want:?}, got {got:?}");
    }

    /// This test detects a bug with upsert when the same CSV, and therefore the
    /// same balances, are imported twice.
    ///
    /// When using the last inserted row id to fetch the balance row, if there
    /// was a conflict which resulted in no rows being inserted/updated the row
    /// id would either be zero if the database connection was reset, or the
    /// last successfully inserted row otherwise. In the first case, this
    /// resulted in an error 'NotFound: the requested resource could not be
    /// found', and in the second case it would result in an unrelated balance
    /// being returned.
    #[tokio::test]
    async fn upsert_balances_twice() {
        let mut store = get_test_store();
        let want = vec![
            Balance {
                id: 1,
                account: "1234-5678-9101-012".to_owned(),
                balance: 123.45,
                date: date!(2025 - 05 - 31),
            },
            Balance {
                id: 2,
                account: "1234-5678-9101-013".to_owned(),
                balance: 234.56,
                date: date!(2025 - 05 - 31),
            },
            Balance {
                id: 3,
                account: "1234-5678-9101-014".to_owned(),
                balance: 345.67,
                date: date!(2025 - 05 - 27),
            },
            Balance {
                id: 4,
                account: "1234-5678-9101-015".to_owned(),
                balance: 567.89,
                date: date!(2025 - 06 - 06),
            },
        ];

        for balance in &want {
            let balance = store
                .upsert(&balance.account, balance.balance, &balance.date)
                .expect("Could not create account balance");
            println!("{balance:#?}");
        }

        let mut got = Vec::new();
        for balance in &want {
            let got_balance = store
                .upsert(&balance.account, balance.balance, &balance.date)
                .expect("Could not create account balance");
            println!("{got_balance:#?}");
            got.push(got_balance);
        }

        assert_eq!(want, got, "want balance {want:?}, got {got:?}");
    }

    #[tokio::test]
    async fn upsert_balance_increments_id() {
        let mut store = get_test_store();
        let want = vec![
            Balance {
                id: 1,
                account: "1234-5678-9101-012".to_owned(),
                balance: 37_337_252_784.63,
                date: date!(2025 - 05 - 31),
            },
            Balance {
                id: 2,
                account: "2345-6789-1011-123".to_owned(),
                balance: 37_337_252_784.63,
                date: date!(2025 - 05 - 31),
            },
        ];

        let mut got = Vec::new();

        for balance in &want {
            let got_balance = store
                .upsert(&balance.account, balance.balance, &balance.date)
                .expect("Could not create account balance");
            got.push(got_balance);
        }

        assert_eq!(want, got, "want balance {want:?}, got {got:?}");
    }

    #[tokio::test]
    async fn upsert_takes_balance_with_latest_date() {
        let mut store = get_test_store();
        let account = "1234-5678-9101-112";
        let test_balances = vec![
            // This entry should be accepted in the first upsert
            Balance {
                id: 1,
                account: account.to_owned(),
                balance: 73_254.89,
                date: date!(2025 - 05 - 30),
            },
            // This entry should overwrite the balance from the first upsert
            // because it is newer
            Balance {
                id: 1,
                account: account.to_owned(),
                balance: 37_337_252_784.63,
                date: date!(2025 - 05 - 31),
            },
            // This entry should be ignored because it is older.
            Balance {
                id: 1,
                account: account.to_owned(),
                balance: 2_727_843.43,
                date: date!(2025 - 05 - 29),
            },
        ];
        let want = vec![
            Balance {
                id: 1,
                account: account.to_owned(),
                balance: 73_254.89,
                date: date!(2025 - 05 - 30),
            },
            Balance {
                id: 1,
                account: account.to_owned(),
                balance: 37_337_252_784.63,
                date: date!(2025 - 05 - 31),
            },
            Balance {
                id: 1,
                account: account.to_owned(),
                balance: 37_337_252_784.63,
                date: date!(2025 - 05 - 31),
            },
        ];

        let mut got = Vec::new();

        for balance in test_balances {
            let got_balance = store
                .upsert(&balance.account, balance.balance, &balance.date)
                .expect("Could not create account balance");
            got.push(got_balance);
        }

        assert_eq!(want, got, "want left");
    }

    #[tokio::test]
    async fn get_all_balances() {
        let mut store = get_test_store();
        let want = vec![
            Balance {
                id: 1,
                account: "1234-5678-9101-012".to_owned(),
                balance: 37_337_252_784.63,
                date: date!(2025 - 05 - 31),
            },
            Balance {
                id: 2,
                account: "2345-6789-1011-123".to_owned(),
                balance: 37_337_252_784.63,
                date: date!(2025 - 05 - 31),
            },
        ];
        for balance in &want {
            store
                .upsert(&balance.account, balance.balance, &balance.date)
                .expect("Could not create account balance");
        }

        let got = store.get_all();

        assert_eq!(Ok(want.clone()), got, "want balance {want:?}, got {got:?}");
    }
}
