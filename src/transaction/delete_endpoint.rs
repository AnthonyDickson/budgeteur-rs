use std::sync::{Arc, Mutex};

use axum::{
    extract::{FromRef, Path, State},
    http::StatusCode,
    response::{Html, IntoResponse},
};
use rusqlite::Connection;

use crate::{
    AppState, Error, alert::AlertTemplate, database_id::TransactionId, shared_templates::render,
};

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
///
/// # Panics
///
/// Panics if the lock for the database connection is already held by the same thread.
pub async fn delete_transaction_endpoint(
    State(state): State<DeleteTransactionState>,
    Path(transaction_id): Path<TransactionId>,
) -> impl IntoResponse {
    let connection = state.db_connection.lock().unwrap();

    match delete_transaction(transaction_id, &connection) {
        Ok(0) => render(
            StatusCode::NOT_FOUND,
            AlertTemplate::error(
                "Could not delete transaction",
                "The transaction could not be found. \
                Try refreshing the page to see if the transaction has already been deleted.",
            ),
        ),
        // The status code has to be 200 OK or HTMX will not delete the table row.
        Ok(_) => Html(EMPTY_TRANSACTION_TABLE_ROW).into_response(),
        Err(error) => {
            tracing::error!("Could not delete transaction {transaction_id}: {error}");
            render(
                StatusCode::INTERNAL_SERVER_ERROR,
                AlertTemplate::error(
                    "Could not delete transaction",
                    "An unexpected error occured. Try again later or check the logs on the server.",
                ),
            )
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
