//! Implements a SQLite backed balance store.
use std::sync::{Arc, Mutex};

use time::Date;

use crate::{
    Error,
    db::{CreateTable, MapRow},
    models::{Balance, UserID},
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
            "CREATE TABLE balance (
                id INTEGER PRIMARY KEY,
                account TEXT NOT NULL,
                balance REAL NOT NULL,
                date TEXT NOT NULL,
                user_id INTEGER NOT NULL,
                FOREIGN KEY(user_id) REFERENCES user(id) ON UPDATE CASCADE ON DELETE CASCADE
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
        let user_id = row.get(offset + 4)?;

        Ok(Balance {
            id,
            account,
            balance,
            date,
            user_id: UserID::new(user_id),
        })
    }
}

impl BalanceStore for SQLiteBalanceStore {
    fn create(&mut self, account: &str, balance: f64, date: &Date) -> Result<Balance, Error> {
        let connection = self
            .connection
            .lock()
            .expect("Could not acquire lock to database");

        let next_id: i64 =
            connection.query_row("SELECT COALESCE(MAX(id), 0) FROM balance", [], |row| {
                row.get(0)
            })?;
        let next_id = next_id + 1;

        connection.execute(
            "INSERT INTO balance (id, account, balance, date, user_id) VALUES (?1, ?2, ?3, ?4, ?5)",
            (next_id, account, balance, date, 1),
        )?;

        Ok(Balance {
            id: next_id,
            account: account.to_owned(),
            balance,
            date: date.to_owned(),
            user_id: UserID::new(1),
        })
    }

    fn get_by_user_id(&self, user_id: UserID) -> Result<Vec<Balance>, Error> {
        self.connection
            .lock()
            .expect("Could not acquire database lock")
            .prepare(
                "SELECT id, account, balance, date, user_id FROM balance WHERE user_id = :user_id",
            )?
            .query_map(
                &[(":user_id", &user_id.as_i64())],
                SQLiteBalanceStore::map_row,
            )?
            .map(|maybe_balance| {
                maybe_balance.map_err(|error| match error {
                    // Code 787 occurs when a FOREIGN KEY constraint failed.
                    rusqlite::Error::SqliteFailure(error, Some(_))
                        if error.extended_code == 787 =>
                    {
                        Error::InvalidUser
                    }
                    error => error.into(),
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod sqlite_balance_store_tests {
    use std::sync::{Arc, Mutex};

    use rusqlite::Connection;
    use time::macros::date;

    use crate::{
        db::CreateTable,
        models::{Balance, PasswordHash, User},
        stores::{BalanceStore, UserStore, sqlite::SQLiteUserStore},
    };

    use super::SQLiteBalanceStore;

    fn get_store_and_user() -> (SQLiteBalanceStore, User) {
        let connection = Connection::open_in_memory().unwrap();
        SQLiteUserStore::create_table(&connection).unwrap();
        SQLiteBalanceStore::create_table(&connection).unwrap();
        let connection = Arc::new(Mutex::new(connection));

        let user = SQLiteUserStore::new(connection.clone())
            .create(
                "foo@bar.baz".parse().unwrap(),
                PasswordHash::from_raw_password("naetoafntseoafunts", 4).unwrap(),
            )
            .unwrap();

        let store = SQLiteBalanceStore::new(connection.clone());

        (store, user)
    }

    #[tokio::test]
    async fn can_create_balance() {
        let (mut store, test_user) = get_store_and_user();
        let want = Balance {
            id: 1,
            account: "1234-5678-9101-012".to_owned(),
            balance: 37_337_252_784.63,
            date: date!(2025 - 05 - 31),
            user_id: test_user.id(),
        };

        let got = store
            .create(&want.account, want.balance, &want.date)
            .expect("Could not create account balance");

        assert_eq!(want, got, "want balance {want:?}, got {got:?}");
    }

    #[tokio::test]
    async fn create_balance_increments_id() {
        let (mut store, test_user) = get_store_and_user();
        let want = vec![
            Balance {
                id: 1,
                account: "1234-5678-9101-012".to_owned(),
                balance: 37_337_252_784.63,
                date: date!(2025 - 05 - 31),
                user_id: test_user.id(),
            },
            Balance {
                id: 2,
                account: "1234-5678-9101-012".to_owned(),
                balance: 37_337_252_784.63,
                date: date!(2025 - 05 - 31),
                user_id: test_user.id(),
            },
        ];

        let mut got = Vec::new();

        for balance in &want {
            let got_balance = store
                .create(&balance.account, balance.balance, &balance.date)
                .expect("Could not create account balance");
            got.push(got_balance);
        }

        assert_eq!(want, got, "want balance {want:?}, got {got:?}");
    }

    #[tokio::test]
    async fn get_balance_by_user_id() {
        let (mut store, test_user) = get_store_and_user();
        let want = vec![
            Balance {
                id: 1,
                account: "1234-5678-9101-012".to_owned(),
                balance: 37_337_252_784.63,
                date: date!(2025 - 05 - 31),
                user_id: test_user.id(),
            },
            Balance {
                id: 2,
                account: "1234-5678-9101-012".to_owned(),
                balance: 37_337_252_784.63,
                date: date!(2025 - 05 - 31),
                user_id: test_user.id(),
            },
        ];
        for balance in &want {
            store
                .create(&balance.account, balance.balance, &balance.date)
                .expect("Could not create account balance");
        }

        let got = store.get_by_user_id(test_user.id());

        assert_eq!(Ok(want.clone()), got, "want balance {want:?}, got {got:?}");
    }
}
