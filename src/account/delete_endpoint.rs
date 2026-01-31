//! Defines the endpoint for deleting an account.

use std::sync::{Arc, Mutex};

use axum::{
    extract::{FromRef, Path, State},
    response::{IntoResponse, Response},
};
use rusqlite::Connection;

use crate::{AppState, Error, account::core::AccountId, alert::Alert};

/// The state needed to delete an account.
#[derive(Debug, Clone)]
pub struct DeleteAccountState {
    /// The database connection for managing account.
    db_connection: Arc<Mutex<Connection>>,
}

impl FromRef<AppState> for DeleteAccountState {
    fn from_ref(state: &AppState) -> Self {
        Self {
            db_connection: state.db_connection.clone(),
        }
    }
}

/// A route handler for deleting an account, responds with an alert.
///
/// # Panics
///
/// Panics if the lock for the database connection is already held by the same thread.
pub async fn delete_account_endpoint(
    State(state): State<DeleteAccountState>,
    Path(account_id): Path<AccountId>,
) -> Response {
    let connection = match state.db_connection.lock() {
        Ok(connection) => connection,
        Err(error) => {
            tracing::error!("could not acquire database lock: {error}");
            return Error::DatabaseLockError.into_alert_response();
        }
    };

    match delete_account(account_id, &connection) {
        // The status code has to be 200 OK or HTMX will not delete the table row.
        Ok(row_affected) if row_affected != 0 => Alert::SuccessSimple {
            message: "Account deleted successfully".to_owned(),
        }
        .into_response(),
        Ok(_) => Error::DeleteMissingAccount.into_alert_response(),
        Err(error) => {
            tracing::error!("Could not delete account {account_id}: {error}");
            error.into_alert_response()
        }
    }
}

type RowsAffected = usize;

fn delete_account(id: AccountId, connection: &Connection) -> Result<RowsAffected, Error> {
    connection
        .execute("DELETE FROM account WHERE id = :id", &[(":id", &id)])
        .map_err(Error::from)
}

#[cfg(test)]
mod tests {
    use rusqlite::{Connection, params};
    use time::macros::date;

    use crate::{
        Error,
        account::{
            Account,
            core::AccountId,
            create_endpoint::{AccountForm, create_account},
            delete_endpoint::delete_account,
            map_row_to_account,
        },
        initialize_db,
    };

    #[test]
    fn test_deletes_account() {
        let connection = Connection::open_in_memory().unwrap();
        initialize_db(&connection).unwrap();
        let account = create_account(
            &AccountForm {
                name: "foo".to_owned(),
                balance: 420.69,
                date: date!(2025 - 11 - 01),
            },
            &connection,
        )
        .unwrap();

        let rows_affected = delete_account(account.id, &connection).unwrap();

        assert_eq!(rows_affected, 1);
        assert_eq!(get_account(account.id, &connection), Err(Error::NotFound))
    }

    #[track_caller]
    fn get_account(id: AccountId, connection: &Connection) -> Result<Account, Error> {
        connection
            .query_one(
                "SELECT id, name, balance, date FROM account WHERE id = ?1",
                params![id],
                map_row_to_account,
            )
            .map_err(Error::from)
    }
}
