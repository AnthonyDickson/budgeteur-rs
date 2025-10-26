//! Defines the endpoint for creating a new transaction.
use std::sync::{Arc, Mutex};

use axum::{
    extract::{FromRef, State},
    http::StatusCode,
    response::IntoResponse,
};
// Must use axum_extra's Form since that parses an empty string as None instead
// of crashing like axum::Form.
use axum_extra::extract::Form;
use axum_htmx::HxRedirect;
use rusqlite::Connection;
use serde::Deserialize;
use time::Date;

use crate::{
    AppState, endpoints,
    tag::TagId,
    transaction::{Transaction, core::create_transaction},
};

/// The state needed to get or create a transaction.
#[derive(Debug, Clone)]
pub struct CreateTransactionState {
    /// The database connection for managing transactions.
    pub db_connection: Arc<Mutex<Connection>>,
}

impl FromRef<AppState> for CreateTransactionState {
    fn from_ref(state: &AppState) -> Self {
        Self {
            db_connection: state.db_connection.clone(),
        }
    }
}

/// The form data for creating a transaction.
#[derive(Debug, Deserialize)]
pub struct TransactionForm {
    /// The value of the transaction in dollars.
    pub amount: f64,
    /// The date when the transaction ocurred.
    pub date: Date,
    /// Text detailing the transaction.
    pub description: String,
    /// The IDs of tags to associate with this transaction.
    #[serde(default)]
    pub tag_id: Option<TagId>,
}

/// A route handler for creating a new transaction, redirects to transactions view on success.
///
/// # Panics
///
/// Panics if the lock for the database connection is already held by the same thread.
pub async fn create_transaction_endpoint(
    State(state): State<CreateTransactionState>,
    Form(form): Form<TransactionForm>,
) -> impl IntoResponse {
    let transaction =
        Transaction::build(form.amount, form.date, form.description).tag_id(form.tag_id);

    let connection = state.db_connection.lock().unwrap();

    if let Err(error) = create_transaction(transaction, &connection) {
        return error.into_response();
    }

    (
        HxRedirect(endpoints::TRANSACTIONS_VIEW.to_owned()),
        StatusCode::SEE_OTHER,
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use axum::{body::Body, extract::State, http::Response, response::IntoResponse};
    use axum_extra::extract::Form;
    use axum_htmx::HX_REDIRECT;
    use rusqlite::Connection;
    use time::OffsetDateTime;

    use crate::{
        Error,
        database_id::DatabaseId,
        db::initialize,
        tag::{TagName, create_tag},
        transaction::{
            Transaction, create_transaction_endpoint,
            create_transaction_endpoint::{CreateTransactionState, TransactionForm},
            map_transaction_row,
        },
    };

    fn get_test_connection() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();
        conn
    }

    #[tokio::test]
    async fn can_create_transaction() {
        let conn = get_test_connection();
        let state = CreateTransactionState {
            db_connection: Arc::new(Mutex::new(conn)),
        };

        let form = TransactionForm {
            description: "test transaction".to_string(),
            amount: 12.3,
            date: OffsetDateTime::now_utc().date(),
            tag_id: None,
        };

        let response = create_transaction_endpoint(State(state.clone()), Form(form))
            .await
            .into_response();

        assert_redirects_to_transactions_view(response);

        // Verify the transaction was actually created by getting it by ID
        // We know the first transaction will have ID 1
        let connection = state.db_connection.lock().unwrap();
        let transaction = get_transaction(1, &connection).unwrap();
        assert_eq!(transaction.amount, 12.3);
        assert_eq!(transaction.description, "test transaction");
    }

    #[tokio::test]
    async fn can_create_transaction_with_tags() {
        let conn = get_test_connection();
        let tag = create_tag(TagName::new_unchecked("Groceries"), &conn).unwrap();
        let state = CreateTransactionState {
            db_connection: Arc::new(Mutex::new(conn)),
        };

        let form = TransactionForm {
            description: "test transaction with tags".to_string(),
            amount: 25.50,
            date: OffsetDateTime::now_utc().date(),
            tag_id: Some(tag.id),
        };
        let response = create_transaction_endpoint(State(state.clone()), Form(form))
            .await
            .into_response();

        assert_redirects_to_transactions_view(response);
        // Verify the transaction was created with tags
        let connection = state.db_connection.lock().unwrap();
        let transaction = get_transaction(1, &connection).unwrap();
        assert_eq!(transaction.tag_id, Some(tag.id));
    }

    #[track_caller]
    fn assert_redirects_to_transactions_view(response: Response<Body>) {
        let location = response
            .headers()
            .get(HX_REDIRECT)
            .expect("expected response to have the header hx-redirect");
        assert_eq!(
            location, "/transactions",
            "got redirect to {location:?}, want redirect to /transactions"
        );
    }

    /// Retrieve a transaction from the database by its `id`.
    ///
    /// # Errors
    /// This function will return a:
    /// - [Error::NotFound] if `id` does not refer to a valid transaction,
    /// - or [Error::SqlError] there is some other SQL error.
    pub fn get_transaction(id: DatabaseId, connection: &Connection) -> Result<Transaction, Error> {
        let transaction = connection
        .prepare(
            "SELECT id, amount, date, description, import_id, tag_id FROM \"transaction\" WHERE id = :id",
        )?
        .query_row(&[(":id", &id)], map_transaction_row)?;

        Ok(transaction)
    }
}
