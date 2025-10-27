use std::sync::{Arc, Mutex};

use askama::Template;
use axum::{
    extract::{FromRef, Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use rusqlite::Connection;
use time::{Date, OffsetDateTime};

use crate::{
    AppState, Error,
    database_id::TransactionId,
    endpoints::{self, format_endpoint},
    navigation::{NavbarTemplate, get_nav_bar},
    not_found::get_404_not_found_response,
    routing::render_internal_server_error,
    shared_templates::render,
    tag::{Tag, get_all_tags},
    timezone::get_local_offset,
    transaction::{Transaction, get_transaction},
};

/// Renders the edit transaction page.
#[derive(Template)]
#[template(path = "views/transaction/edit.html")]
struct EditTransactionPageTemplate<'a> {
    nav_bar: NavbarTemplate<'a>,
    edit_transaction_endpoint: &'a str,
    max_date: Date,
    transaction: Transaction,
    available_tags: Vec<Tag>,
}

/// The state needed for the edit transaction page.
#[derive(Debug, Clone)]
pub struct EditTransactionPageState {
    /// The local timezone as a canonical timezone name, e.g. "Pacific/Auckland".
    pub local_timezone: String,
    /// The database connection for accessing tags.
    pub db_connection: Arc<Mutex<Connection>>,
}

impl FromRef<AppState> for EditTransactionPageState {
    fn from_ref(state: &AppState) -> Self {
        Self {
            local_timezone: state.local_timezone.clone(),
            db_connection: state.db_connection.clone(),
        }
    }
}

/// Renders the page for editing a transaction.
///
/// # Panics
///
/// Panics if the lock for the database connection is already held by the same thread.
pub async fn get_edit_transaction_page(
    State(state): State<EditTransactionPageState>,
    Path(transaction_id): Path<TransactionId>,
) -> Response {
    let nav_bar = get_nav_bar(endpoints::EDIT_TRANSACTION_VIEW);

    let connection = state
        .db_connection
        .lock()
        .expect("Could not acquire database lock");

    let transaction = match get_transaction(transaction_id, &connection) {
        Ok(transaction) => transaction,
        Err(Error::NotFound) => {
            return get_404_not_found_response();
        }
        Err(error) => {
            tracing::error!("Failed to retrieve transaction {transaction_id}: {error}");
            return render_internal_server_error(Default::default());
        }
    };

    let available_tags = match get_all_tags(&connection) {
        Ok(tags) => tags,
        Err(error) => {
            tracing::error!("Failed to retrieve tags for new transaction page: {error}");
            return render_internal_server_error(Default::default());
        }
    };

    let local_timezone = match get_local_offset(&state.local_timezone) {
        Some(offset) => offset,
        None => return Error::InvalidTimezoneError(state.local_timezone).into_response(),
    };

    render(
        StatusCode::OK,
        EditTransactionPageTemplate {
            nav_bar,
            edit_transaction_endpoint: &format_endpoint(
                endpoints::EDIT_TRANSACTION_VIEW,
                transaction_id,
            ),
            max_date: OffsetDateTime::now_utc().to_offset(local_timezone).date(),
            transaction,
            available_tags,
        },
    )
}
