use std::sync::{Arc, Mutex};

use axum::{
    debug_handler,
    extract::{FromRef, Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use axum_extra::extract::Form;
use axum_htmx::HxRedirect;
use rusqlite::{Connection, params};
use serde::Deserialize;
use time::{Date, OffsetDateTime};

use crate::{
    AppState, Error, database_id::TransactionId, endpoints, tag::TagId, timezone::get_local_offset,
};

/// The state needed to edit a transaction.
#[derive(Debug, Clone)]
pub struct EditTransactionState {
    /// The database connection for managing transactions.
    pub db_connection: Arc<Mutex<Connection>>,
    /// The local timezone as a canonical timezone name, e.g. "Pacific/Auckland".
    pub local_timezone: String,
}

impl FromRef<AppState> for EditTransactionState {
    fn from_ref(state: &AppState) -> Self {
        Self {
            db_connection: state.db_connection.clone(),
            local_timezone: state.local_timezone.clone(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct EditTransactionForm {
    amount: f64,
    date: Date,
    description: String,
    tag_id: Option<TagId>,
}

#[derive(Debug, Deserialize)]
pub struct QueryParams {
    redirect_url: Option<String>,
}

#[debug_handler]
pub async fn edit_tranction_endpoint(
    State(state): State<EditTransactionState>,
    Path(transaction_id): Path<TransactionId>,
    Query(query_params): Query<QueryParams>,
    Form(form): Form<EditTransactionForm>,
) -> Response {
    let Some(local_timezone) = get_local_offset(&state.local_timezone) else {
        tracing::error!("Invalid timezone {}", state.local_timezone);

        return Error::InvalidTimezoneError(state.local_timezone).into_alert_response();
    };
    let now_local_time = OffsetDateTime::now_utc().to_offset(local_timezone);

    if form.date > now_local_time.date() {
        tracing::error!(
            "Tried to set the date of a transaction to a future date {}",
            form.date
        );
        return Error::FutureDate(form.date).into_alert_response();
    }

    let connection = match state.db_connection.lock() {
        Ok(connection) => connection,
        Err(error) => {
            tracing::error!("could not acquire database lock: {error}");
            return Error::DatabaseLockError.into_alert_response();
        }
    };

    match update_transaction(transaction_id, &form, &connection) {
        // The status code has to be 200 OK or HTMX will not delete the table row.
        Ok(row_affected) if row_affected != 0 => {}
        Ok(_) => {
            tracing::error!(
                "Could not update transaction {transaction_id}: update returned zero rows affected"
            );
            return Error::UpdateMissingTransaction.into_alert_response();
        }
        Err(error) => {
            tracing::error!("Could not delete transaction {transaction_id}: {error}");
            return error.into_alert_response();
        }
    };

    let redirect_url = query_params
        .redirect_url
        .unwrap_or(endpoints::TRANSACTIONS_VIEW.to_owned());

    (HxRedirect(redirect_url), StatusCode::SEE_OTHER).into_response()
}

type RowsAffected = usize;

fn update_transaction(
    id: TransactionId,
    transaction: &EditTransactionForm,
    connection: &Connection,
) -> Result<RowsAffected, Error> {
    connection
        .execute(
            "UPDATE \"transaction\"
        SET \
            amount = ?1, \
            date = ?2, \
            description = ?3, \
            tag_id = ?4 \
        WHERE id = ?5;",
            params![
                transaction.amount,
                transaction.date,
                transaction.description,
                transaction.tag_id,
                id,
            ],
        )
        .map_err(Error::from)
}

#[cfg(test)]
mod test {
    use std::sync::{Arc, Mutex};

    use axum::{
        extract::{Path, Query, State},
        http::{HeaderValue, StatusCode},
    };
    use axum_extra::extract::Form;
    use axum_htmx::HX_REDIRECT;
    use rusqlite::Connection;
    use time::macros::date;

    use crate::{
        initialize_db,
        tag::create_tag,
        transaction::{
            Transaction, create_transaction,
            edit_endpoint::{
                EditTransactionForm, EditTransactionState, QueryParams, edit_tranction_endpoint,
            },
            get_transaction,
        },
    };

    #[tokio::test]
    async fn can_update_transaction() {
        let conn = must_create_test_connection();
        let tag = create_tag(
            "Foo".parse().expect("could not create test tag name"),
            &conn,
        )
        .expect("could not create test tag");
        create_transaction(
            Transaction::build(1.23, date!(2025 - 10 - 27), "test").tag_id(Some(tag.id)),
            &conn,
        )
        .expect("could not create test transaction");
        let state = EditTransactionState {
            db_connection: Arc::new(Mutex::new(conn)),
            local_timezone: "Etc/UTC".to_owned(),
        };
        let want_transaction = Transaction {
            id: 1,
            amount: 3.21,
            date: date!(2025 - 10 - 28),
            description: "foo".to_owned(),
            import_id: None,
            tag_id: None,
        };
        let form = EditTransactionForm {
            amount: want_transaction.amount,
            date: want_transaction.date,
            description: want_transaction.description.clone(),
            tag_id: want_transaction.tag_id,
        };
        let redirect_url = "foo/bar?page=123&per_page=20".to_owned();

        let response = edit_tranction_endpoint(
            State(state.clone()),
            Path(want_transaction.id),
            Query(QueryParams {
                redirect_url: Some(redirect_url.clone()),
            }),
            Form(form),
        )
        .await;

        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        assert_eq!(
            response.headers().get(HX_REDIRECT),
            Some(&HeaderValue::from_str(&redirect_url).unwrap())
        );
        let got_transaction = get_transaction(
            want_transaction.id,
            &state.db_connection.lock().expect("could not fetch string"),
        )
        .expect("could not get test transaction");
        assert_eq!(want_transaction, got_transaction);
    }

    fn must_create_test_connection() -> Connection {
        let connection =
            Connection::open_in_memory().expect("could not create in-memory SQLite database");
        initialize_db(&connection).expect("could not initialize test DB");

        connection
    }
}
