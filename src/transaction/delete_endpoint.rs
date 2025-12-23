use std::sync::{Arc, Mutex};

use axum::{
    extract::{FromRef, Path, State},
    response::{Html, IntoResponse, Response},
};
use rusqlite::Connection;

use crate::{AppState, Error, database_id::TransactionId};

/// The state needed to delete a transaction.
#[derive(Debug, Clone)]
pub struct DeleteTransactionState {
    /// The database connection for managing transactions.
    db_connection: Arc<Mutex<Connection>>,
}

impl FromRef<AppState> for DeleteTransactionState {
    fn from_ref(state: &AppState) -> Self {
        Self {
            db_connection: state.db_connection.clone(),
        }
    }
}

const EMPTY_TRANSACTION_TABLE_ROW: &str =
    include_str!("./../../templates/partials/transaction_table_row_empty.html");

/// A route handler for deleting a transaction, responds with an alert.
pub async fn delete_transaction_endpoint(
    State(state): State<DeleteTransactionState>,
    Path(transaction_id): Path<TransactionId>,
) -> Response {
    let connection = match state.db_connection.lock() {
        Ok(connection) => connection,
        Err(error) => {
            tracing::error!("could not acquire database lock: {error}");
            return Error::DatabaseLockError.into_alert_response();
        }
    };

    match delete_transaction(transaction_id, &connection) {
        // The status code has to be 200 OK or HTMX will not delete the table row.
        Ok(row_affected) if row_affected != 0 => Html(EMPTY_TRANSACTION_TABLE_ROW).into_response(),
        Ok(_) => Error::DeleteMissingTransaction.into_alert_response(),
        Err(error) => {
            tracing::error!("Could not delete transaction {transaction_id}: {error}");
            error.into_alert_response()
        }
    }
}

type RowsAffected = usize;

fn delete_transaction(id: TransactionId, connection: &Connection) -> Result<RowsAffected, Error> {
    connection
        .execute(
            "DELETE FROM \"transaction\" WHERE id = :id",
            &[(":id", &id)],
        )
        .map_err(|err| err.into())
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;
    use time::macros::date;

    use crate::{
        Error, initialize_db,
        transaction::{
            TransactionBuilder, create_transaction, delete_endpoint::delete_transaction,
            get_transaction,
        },
    };

    #[test]
    fn test_deletes_transaction() {
        let connection = Connection::open_in_memory().unwrap();
        initialize_db(&connection).unwrap();
        let transaction = create_transaction(
            TransactionBuilder {
                amount: 1.23,
                date: date!(2025 - 10 - 26),
                description: "Test".to_owned(),
                import_id: None,
                tag_id: None,
            },
            &connection,
        )
        .unwrap();

        let rows_affected = delete_transaction(transaction.id, &connection).unwrap();

        assert_eq!(rows_affected, 1);
        assert_eq!(
            get_transaction(transaction.id, &connection),
            Err(Error::NotFound)
        )
    }
}
