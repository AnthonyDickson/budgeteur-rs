//! Defines the endpoint for creating a new transaction.
use std::sync::{Arc, Mutex};

use axum::{
    extract::{FromRef, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
// Must use axum_extra's Form since that parses an empty string as None instead
// of crashing like axum::Form.
use axum_extra::extract::Form;
use axum_htmx::HxRedirect;
use rusqlite::Connection;
use serde::Deserialize;
use time::{Date, OffsetDateTime};

use crate::{
    AppState, Error, endpoints,
    tag::TagId,
    timezone::get_local_offset,
    transaction::{Transaction, core::create_transaction},
};

/// The state needed to get or create a transaction.
#[derive(Debug, Clone)]
pub struct CreateTransactionState {
    /// The database connection for managing transactions.
    pub db_connection: Arc<Mutex<Connection>>,
    /// The local timezone as a canonical timezone name, e.g. "Pacific/Auckland".
    pub local_timezone: String,
}

impl FromRef<AppState> for CreateTransactionState {
    fn from_ref(state: &AppState) -> Self {
        Self {
            db_connection: state.db_connection.clone(),
            local_timezone: state.local_timezone.clone(),
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
pub async fn create_transaction_endpoint(
    State(state): State<CreateTransactionState>,
    Form(form): Form<TransactionForm>,
) -> Response {
    let Some(local_timezone) = get_local_offset(&state.local_timezone) else {
        tracing::error!("Invalid timezone {}", state.local_timezone);
        return Error::InvalidTimezoneError(state.local_timezone).into_alert_response();
    };

    let now_local_time = OffsetDateTime::now_utc().to_offset(local_timezone);

    if form.date > now_local_time.date() {
        tracing::error!(
            "Tried to perform an operation with a future date (e.g., create a transaction)"
        );

        return Error::FutureDate(form.date).into_alert_response();
    }

    let transaction =
        Transaction::build(form.amount, form.date, &form.description).tag_id(form.tag_id);

    let connection = match state.db_connection.lock() {
        Ok(connection) => connection,
        Err(error) => {
            tracing::error!("could not acquire database lock: {error}");
            return Error::DatabaseLockError.into_alert_response();
        }
    };

    if let Err(error) = create_transaction(transaction, &connection) {
        tracing::error!("could not create transaction: {error}");

        return error.into_alert_response();
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
        db::initialize,
        tag::{TagName, create_tag},
        transaction::{
            create_endpoint::{CreateTransactionState, TransactionForm},
            create_transaction_endpoint, get_transaction,
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
            local_timezone: "Etc/UTC".to_owned(),
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
            local_timezone: "Etc/UTC".to_owned(),
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
}
