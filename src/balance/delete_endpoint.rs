//! Defines the endpoint for deleting an account balance.

use std::sync::{Arc, Mutex};

use axum::{
    extract::{FromRef, Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use rusqlite::Connection;

use crate::{
    AppState, Error, alert::AlertTemplate, database_id::DatabaseId, shared_templates::render,
};

/// The state needed to delete an account balance.
#[derive(Debug, Clone)]
pub struct DeleteAccountState {
    /// The database connection for managing account balances.
    db_connection: Arc<Mutex<Connection>>,
}

impl FromRef<AppState> for DeleteAccountState {
    fn from_ref(state: &AppState) -> Self {
        Self {
            db_connection: state.db_connection.clone(),
        }
    }
}

/// A route handler for deleting an account balance, responds with an alert.
///
/// # Panics
///
/// Panics if the lock for the database connection is already held by the same thread.
pub async fn delete_account_endpoint(
    State(state): State<DeleteAccountState>,
    Path(account_id): Path<DatabaseId>,
) -> impl IntoResponse {
    let connection = match state.db_connection.lock() {
        Ok(connection) => connection,
        Err(error) => {
            tracing::error!("Could not acquire database lock: {error}");
            return render(
                StatusCode::INTERNAL_SERVER_ERROR,
                AlertTemplate::error(
                    "Could not delete account",
                    "An unexpected error occured. Try again later or check the logs on the server.",
                ),
            );
        }
    };

    match delete_account(account_id, &connection) {
        Ok(0) => render(
            StatusCode::NOT_FOUND,
            AlertTemplate::error(
                "Could not delete account",
                "The account could not be found. \
                Try refreshing the page to see if the account has already been deleted.",
            ),
        ),
        // The status code has to be 200 OK or HTMX will not delete the table row.
        Ok(_) => render(
            StatusCode::OK,
            AlertTemplate::success("Account deleted successfully", ""),
        ),
        Err(error) => {
            tracing::error!("Could not delete account {account_id}: {error}");
            render(
                StatusCode::INTERNAL_SERVER_ERROR,
                AlertTemplate::error(
                    "Could not delete account",
                    "An unexpected error occured. Try again later or check the logs on the server.",
                ),
            )
        }
    }
}

type RowsAffected = usize;

fn delete_account(id: DatabaseId, connection: &Connection) -> Result<RowsAffected, Error> {
    connection
        .execute("DELETE FROM balance WHERE id = :id", &[(":id", &id)])
        .map_err(Error::from)
}

#[cfg(test)]
mod tests {
    use rusqlite::{Connection, params};
    use time::macros::date;

    use crate::{
        Error,
        balance::{
            Balance,
            create_endpoint::{AccountBalanceForm, create_account_balance},
            delete_endpoint::delete_account,
            map_row_to_balance,
        },
        database_id::DatabaseId,
        initialize_db,
    };

    #[test]
    fn test_deletes_account() {
        let connection = Connection::open_in_memory().unwrap();
        initialize_db(&connection).unwrap();
        let account = create_account_balance(
            &AccountBalanceForm {
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
    fn get_account(id: DatabaseId, connection: &Connection) -> Result<Balance, Error> {
        connection
            .query_one(
                "SELECT id, account, balance, date FROM balance WHERE id = ?1",
                params![id],
                map_row_to_balance,
            )
            .map_err(Error::from)
    }
}
