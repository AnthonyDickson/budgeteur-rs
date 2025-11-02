use rusqlite::Connection;
use time::Date;

use crate::{
    Error,
    account::{Account, map_row_to_account},
};

/// An account imported from a CSV.
#[derive(Debug, PartialEq)]
pub struct ImportAccount {
    /// The account name/number.
    pub name: String,
    /// The balance in the account.
    pub balance: f64,
    /// The date the balance is for.
    pub date: Date,
}

pub fn upsert_account(
    imported_account: &ImportAccount,
    connection: &Connection,
) -> Result<Account, Error> {
    // First, try the upsert with RETURNING
    let maybe_account = connection
        .prepare(
            "INSERT INTO account (name, balance, date)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(name) DO UPDATE SET
                 balance = excluded.balance,
                 date = excluded.date
             WHERE excluded.date > account.date
             RETURNING id, name, balance, date",
        )?
        .query_row(
            (
                &imported_account.name,
                imported_account.balance,
                imported_account.date,
            ),
            map_row_to_account,
        );

    match maybe_account {
        Ok(account) => Ok(account),
        Err(rusqlite::Error::QueryReturnedNoRows) => {
            // No rows returned means the WHERE condition wasn't met
            // (trying to insert older data), so fetch the existing record
            connection
                .prepare("SELECT id, name, balance, date FROM account WHERE name = ?1")?
                .query_row([&imported_account.name], map_row_to_account)
                .map_err(Error::from)
        }
        Err(error) => Err(Error::from(error)),
    }
}

#[cfg(test)]
mod upsert_account_tests {
    use rusqlite::Connection;
    use time::macros::date;

    use crate::{
        account::{Account, create_account_table},
        csv_import::account::{ImportAccount, upsert_account},
    };

    #[tokio::test]
    async fn can_upsert_account() {
        let connection =
            Connection::open_in_memory().expect("Could not initialise in-memory SQLite database");
        create_account_table(&connection).expect("Could not create account table");
        let want = Account {
            id: 1,
            name: "1234-5678-9101-012".to_owned(),
            balance: 37_337_252_784.63,
            date: date!(2025 - 05 - 31),
        };

        let got = upsert_account(
            &ImportAccount {
                name: want.name.clone(),
                balance: want.balance,
                date: want.date,
            },
            &connection,
        )
        .expect("Could not create account");

        assert_eq!(want, got, "want account {want:?}, got {got:?}");
    }

    /// This test detects a bug with upsert when the same CSV, and therefore the
    /// same account, are imported twice.
    ///
    /// When using the last inserted row id to fetch the account row, if there
    /// was a conflict which resulted in no rows being inserted/updated the row
    /// id would either be zero if the database connection was reset, or the
    /// last successfully inserted row otherwise. In the first case, this
    /// resulted in an error 'NotFound: the requested resource could not be
    /// found', and in the second case it would result in an unrelated account
    /// being returned.
    #[tokio::test]
    async fn upsert_accounts_twice() {
        let want = vec![
            Account {
                id: 1,
                name: "1234-5678-9101-012".to_owned(),
                balance: 123.45,
                date: date!(2025 - 05 - 31),
            },
            Account {
                id: 2,
                name: "1234-5678-9101-013".to_owned(),
                balance: 234.56,
                date: date!(2025 - 05 - 31),
            },
            Account {
                id: 3,
                name: "1234-5678-9101-014".to_owned(),
                balance: 345.67,
                date: date!(2025 - 05 - 27),
            },
            Account {
                id: 4,
                name: "1234-5678-9101-015".to_owned(),
                balance: 567.89,
                date: date!(2025 - 06 - 06),
            },
        ];
        let connection =
            Connection::open_in_memory().expect("Could not initialise in-memory SQLite database");
        create_account_table(&connection).expect("Could not create account table");

        for account in &want {
            upsert_account(
                &ImportAccount {
                    name: account.name.clone(),
                    balance: account.balance,
                    date: account.date,
                },
                &connection,
            )
            .expect("Could not create account");
        }

        let mut got = Vec::new();
        for account in &want {
            let got_account = upsert_account(
                &ImportAccount {
                    name: account.name.clone(),
                    balance: account.balance,
                    date: account.date,
                },
                &connection,
            )
            .expect("Could not create account");
            got.push(got_account);
        }

        assert_eq!(want, got, "want account {want:?}, got {got:?}");
    }

    #[tokio::test]
    async fn upsert_account_increments_id() {
        let want = vec![
            Account {
                id: 1,
                name: "1234-5678-9101-012".to_owned(),
                balance: 37_337_252_784.63,
                date: date!(2025 - 05 - 31),
            },
            Account {
                id: 2,
                name: "2345-6789-1011-123".to_owned(),
                balance: 37_337_252_784.63,
                date: date!(2025 - 05 - 31),
            },
        ];
        let connection =
            Connection::open_in_memory().expect("Could not initialise in-memory SQLite database");
        create_account_table(&connection).expect("Could not create accounts table");

        let mut got = Vec::new();

        for account in &want {
            let got_account = upsert_account(
                &ImportAccount {
                    name: account.name.clone(),
                    balance: account.balance,
                    date: account.date,
                },
                &connection,
            )
            .expect("Could not create account");
            got.push(got_account);
        }

        assert_eq!(want, got, "want accunt {want:?}, got {got:?}");
    }

    #[tokio::test]
    async fn use_account_with_latest_date() {
        let connection =
            Connection::open_in_memory().expect("Could not initialise in-memory SQLite database");
        create_account_table(&connection).expect("Could not create account table");
        let account = "1234-5678-9101-112";
        let test_accounts = vec![
            // This entry should be accepted in the first upsert
            Account {
                id: 1,
                name: account.to_owned(),
                balance: 73_254.89,
                date: date!(2025 - 05 - 30),
            },
            // This entry should overwrite the balance from the first upsert
            // because it is newer
            Account {
                id: 1,
                name: account.to_owned(),
                balance: 37_337_252_784.63,
                date: date!(2025 - 05 - 31),
            },
            // This entry should be ignored because it is older.
            Account {
                id: 1,
                name: account.to_owned(),
                balance: 2_727_843.43,
                date: date!(2025 - 05 - 29),
            },
        ];
        let want = vec![
            Account {
                id: 1,
                name: account.to_owned(),
                balance: 73_254.89,
                date: date!(2025 - 05 - 30),
            },
            Account {
                id: 1,
                name: account.to_owned(),
                balance: 37_337_252_784.63,
                date: date!(2025 - 05 - 31),
            },
            Account {
                id: 1,
                name: account.to_owned(),
                balance: 37_337_252_784.63,
                date: date!(2025 - 05 - 31),
            },
        ];

        let mut got = Vec::new();

        for account in test_accounts {
            let got_account = upsert_account(
                &ImportAccount {
                    name: account.name.clone(),
                    balance: account.balance,
                    date: account.date,
                },
                &connection,
            )
            .expect("Could not create account");
            got.push(got_account);
        }

        assert_eq!(want, got, "want left");
    }
}
