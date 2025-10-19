use rusqlite::Connection;
use time::Date;

use crate::{
    Error,
    balances::{Balance, map_row_to_balance},
};

/// An account and balance imported from a CSV.
#[derive(Debug, PartialEq)]
pub struct ImportBalance {
    /// The account name/number.
    pub account: String,
    /// The balance in the account.
    pub balance: f64,
    /// The date the balance is for.
    pub date: Date,
}

pub fn upsert_balance(
    imported_balance: &ImportBalance,
    connection: &Connection,
) -> Result<Balance, Error> {
    // First, try the upsert with RETURNING
    let maybe_balance = connection
        .prepare(
            "INSERT INTO balance (account, balance, date)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(account) DO UPDATE SET
                 balance = excluded.balance,
                 date = excluded.date
             WHERE excluded.date > balance.date
             RETURNING id, account, balance, date",
        )?
        .query_row(
            (
                &imported_balance.account,
                imported_balance.balance,
                imported_balance.date,
            ),
            map_row_to_balance,
        );

    match maybe_balance {
        Ok(balance) => Ok(balance),
        Err(rusqlite::Error::QueryReturnedNoRows) => {
            // No rows returned means the WHERE condition wasn't met
            // (trying to insert older data), so fetch the existing record
            connection
                .prepare("SELECT id, account, balance, date FROM balance WHERE account = ?1")?
                .query_row([&imported_balance.account], map_row_to_balance)
                .map_err(Error::from)
        }
        Err(error) => Err(Error::from(error)),
    }
}

#[cfg(test)]
mod upsert_balance_tests {
    use rusqlite::Connection;
    use time::macros::date;

    use crate::{
        balances::{Balance, create_balance_table},
        csv_import::balance::{ImportBalance, upsert_balance},
    };

    #[tokio::test]
    async fn can_upsert_balance() {
        let connection =
            Connection::open_in_memory().expect("Could not initialise in-memory SQLite database");
        create_balance_table(&connection).expect("Could not create balances table");
        let want = Balance {
            id: 1,
            account: "1234-5678-9101-012".to_owned(),
            balance: 37_337_252_784.63,
            date: date!(2025 - 05 - 31),
        };

        let got = upsert_balance(
            &ImportBalance {
                account: want.account.clone(),
                balance: want.balance,
                date: want.date,
            },
            &connection,
        )
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
        let connection =
            Connection::open_in_memory().expect("Could not initialise in-memory SQLite database");
        create_balance_table(&connection).expect("Could not create balances table");

        for balance in &want {
            upsert_balance(
                &ImportBalance {
                    account: balance.account.clone(),
                    balance: balance.balance,
                    date: balance.date,
                },
                &connection,
            )
            .expect("Could not create account balance");
        }

        let mut got = Vec::new();
        for balance in &want {
            let got_balance = upsert_balance(
                &ImportBalance {
                    account: balance.account.clone(),
                    balance: balance.balance,
                    date: balance.date,
                },
                &connection,
            )
            .expect("Could not create account balance");
            got.push(got_balance);
        }

        assert_eq!(want, got, "want balance {want:?}, got {got:?}");
    }

    #[tokio::test]
    async fn upsert_balance_increments_id() {
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
        let connection =
            Connection::open_in_memory().expect("Could not initialise in-memory SQLite database");
        create_balance_table(&connection).expect("Could not create balances table");

        let mut got = Vec::new();

        for balance in &want {
            let got_balance = upsert_balance(
                &ImportBalance {
                    account: balance.account.clone(),
                    balance: balance.balance,
                    date: balance.date,
                },
                &connection,
            )
            .expect("Could not create account balance");
            got.push(got_balance);
        }

        assert_eq!(want, got, "want balance {want:?}, got {got:?}");
    }

    #[tokio::test]
    async fn upsert_takes_balance_with_latest_date() {
        let connection =
            Connection::open_in_memory().expect("Could not initialise in-memory SQLite database");
        create_balance_table(&connection).expect("Could not create balances table");
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
            let got_balance = upsert_balance(
                &ImportBalance {
                    account: balance.account.clone(),
                    balance: balance.balance,
                    date: balance.date,
                },
                &connection,
            )
            .expect("Could not create account balance");
            got.push(got_balance);
        }

        assert_eq!(want, got, "want left");
    }
}
