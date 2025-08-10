//! This files defines the routes for the transaction type.

use axum::{
    Form, Json,
    extract::{Path, State},
    http::{StatusCode, Uri},
    response::IntoResponse,
};
use axum_htmx::HxRedirect;
use serde::Deserialize;
use time::Date;

use crate::{
    models::{DatabaseID, Transaction},
    state::TransactionState,
    transaction::{create_transaction as create_transaction_db, get_transaction},
};

use super::endpoints;

/// The form data for creating a transaction.
#[derive(Debug, Deserialize)]
pub struct TransactionForm {
    /// The value of the transaction in dollars.
    pub amount: f64,
    /// The date when the transaction ocurred.
    pub date: Date,
    /// Text detailing the transaction.
    pub description: String,
    /// The ID of the category to assign the transaction to.
    ///
    /// Zero should be interpreted as `None`.
    pub category_id: DatabaseID,
}

/// A route handler for creating a new transaction, returns [TransactionRow] as a [Response] on success.
///
/// # Panics
///
/// Panics if the lock for the database connection is already held by the same thread.
pub async fn create_transaction(
    State(state): State<TransactionState>,
    Form(data): Form<TransactionForm>,
) -> impl IntoResponse {
    // HACK: Zero is used as a sentinel value for None. Currently, options do not work with empty
    // form values. For example, the URL encoded form "num=" will return an error.
    let category = match data.category_id {
        0 => None,
        id => Some(id),
    };

    let transaction = Transaction::build(data.amount)
        .description(&data.description)
        .category(category)
        .date(data.date);

    let transaction = match transaction {
        Ok(transaction) => transaction,
        Err(e) => return e.into_response(),
    };

    let connection = state.db_connection.lock().unwrap();
    match create_transaction_db(transaction, &connection) {
        Ok(_) => {}
        Err(e) => return e.into_response(),
    }

    (
        HxRedirect(Uri::from_static(endpoints::TRANSACTIONS_VIEW)),
        StatusCode::SEE_OTHER,
    )
        .into_response()
}

/// A route handler for getting a transaction by its database ID.
///
/// This function will return the status code 404 if the requested resource does not exist (e.g., not created yet).
///
/// # Panics
///
/// Panics if the lock for the database connection is already held by the same thread.
pub async fn get_transaction_endpoint(
    State(state): State<TransactionState>,
    Path(transaction_id): Path<DatabaseID>,
) -> impl IntoResponse {
    let connection = state.db_connection.lock().unwrap();
    get_transaction(transaction_id, &connection)
        .map(|transaction| (StatusCode::OK, Json(transaction)))
}

#[cfg(test)]
mod transaction_tests {
    use std::sync::{Arc, Mutex};

    use askama_axum::IntoResponse;
    use axum::{
        Form,
        body::Body,
        extract::{Path, State},
        http::{Response, StatusCode},
    };
    use axum_htmx::HX_REDIRECT;
    use time::OffsetDateTime;

    use crate::{
        db::initialize,
        models::{Transaction, TransactionBuilder},
        routes::transaction::{TransactionForm, create_transaction, get_transaction_endpoint},
        state::TransactionState,
        transaction::{create_transaction as create_transaction_db, get_transaction},
    };
    use rusqlite::Connection;

    fn get_test_connection() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();
        conn
    }

    #[tokio::test]
    async fn can_create_transaction() {
        let conn = get_test_connection();
        let state = TransactionState {
            db_connection: Arc::new(Mutex::new(conn)),
        };

        let form = TransactionForm {
            description: "test transaction".to_string(),
            amount: 12.3,
            date: OffsetDateTime::now_utc().date(),
            category_id: 0, // 0 means no category
        };

        let response = create_transaction(State(state.clone()), Form(form))
            .await
            .into_response();

        assert_redirects_to_transactions_view(response);

        // Verify the transaction was actually created by getting it by ID
        // We know the first transaction will have ID 1
        let connection = state.db_connection.lock().unwrap();
        let transaction = get_transaction(1, &connection).unwrap();
        assert_eq!(transaction.amount(), 12.3);
        assert_eq!(transaction.description(), "test transaction");
    }

    #[tokio::test]
    async fn can_get_transaction() {
        let conn = get_test_connection();
        let state = TransactionState {
            db_connection: Arc::new(Mutex::new(conn)),
        };

        // Create a transaction first
        let transaction = {
            let connection = state.db_connection.lock().unwrap();
            create_transaction_db(
                TransactionBuilder::new(13.34).description("foobar"),
                &connection,
            )
            .unwrap()
        };

        let response = get_transaction_endpoint(State(state), Path(transaction.id()))
            .await
            .into_response();

        assert_eq!(response.status(), StatusCode::OK);
        let json_response = extract_from_json(response).await;
        assert_eq!(json_response, transaction);
    }

    async fn extract_from_json(response: Response<Body>) -> Transaction {
        let body = response.into_body();
        let body = axum::body::to_bytes(body, usize::MAX).await.unwrap();

        serde_json::from_slice(&body).unwrap()
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
}
