use std::sync::{Arc, Mutex};

use askama::Template;
use axum::{
    extract::{FromRef, Path, Query, State},
    http::StatusCode,
    response::Response,
};
use rusqlite::Connection;
use serde::Deserialize;
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
    edit_transaction_url: &'a str,
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

#[derive(Debug, Deserialize)]
pub struct QueryParams {
    redirect_url: Option<String>,
}

/// Renders the page for editing a transaction.
///
/// # Panics
///
/// Panics if the lock for the database connection is already held by the same thread.
pub async fn get_edit_transaction_page(
    State(state): State<EditTransactionPageState>,
    Path(transaction_id): Path<TransactionId>,
    Query(query_params): Query<QueryParams>,
) -> Response {
    let nav_bar = get_nav_bar(endpoints::EDIT_TRANSACTION_VIEW);

    let connection = match state.db_connection.lock() {
        Ok(connection) => connection,
        Err(error) => {
            tracing::error!("Could not acquire database lock: {error}");
            return render_internal_server_error(Default::default());
        }
    };

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

    let Some(local_timezone) = get_local_offset(&state.local_timezone) else {
        tracing::error!("Failed to get local timezone offset");
        return render_internal_server_error(Default::default());
    };

    let base_url = format_endpoint(endpoints::EDIT_TRANSACTION_VIEW, transaction_id);
    let edit_transaction_url = match query_params.redirect_url {
        Some(redirect_url) => format!("{base_url}?redirect_url={redirect_url}"),
        None => base_url,
    };

    render(
        StatusCode::OK,
        EditTransactionPageTemplate {
            nav_bar,
            edit_transaction_url: &edit_transaction_url,
            max_date: OffsetDateTime::now_utc().to_offset(local_timezone).date(),
            transaction,
            available_tags,
        },
    )
}
