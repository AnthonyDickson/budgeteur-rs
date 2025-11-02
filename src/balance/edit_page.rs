use std::sync::{Arc, Mutex};

use askama::Template;
use axum::{
    extract::{FromRef, Path, State},
    http::StatusCode,
    response::Response,
};
use rusqlite::Connection;
use time::{Date, OffsetDateTime};

use crate::{
    AppState, Error,
    balance::{Balance, map_row_to_balance},
    database_id::DatabaseId,
    endpoints::{self, format_endpoint},
    internal_server_error::render_internal_server_error,
    navigation::{NavbarTemplate, get_nav_bar},
    not_found::get_404_not_found_response,
    shared_templates::render,
    timezone::get_local_offset,
};

/// Renders the edit account page.
#[derive(Template)]
#[template(path = "views/balance/edit.html")]
struct EditAccountPageTemplate<'a> {
    nav_bar: NavbarTemplate<'a>,
    edit_url: String,
    max_date: Date,
    account: Balance,
}

/// The state needed for the edit account page.
#[derive(Debug, Clone)]
pub struct EditAccountPageState {
    /// The local timezone as a canonical timezone name, e.g. "Pacific/Auckland".
    pub local_timezone: String,
    /// The database connection for accessing tags.
    pub db_connection: Arc<Mutex<Connection>>,
}

impl FromRef<AppState> for EditAccountPageState {
    fn from_ref(state: &AppState) -> Self {
        Self {
            local_timezone: state.local_timezone.clone(),
            db_connection: state.db_connection.clone(),
        }
    }
}

/// Renders the page for editing an account.
pub async fn get_edit_account_page(
    State(state): State<EditAccountPageState>,
    Path(account_id): Path<DatabaseId>,
) -> Response {
    let nav_bar = get_nav_bar(endpoints::EDIT_BALANCE_VIEW);

    let connection = match state.db_connection.lock() {
        Ok(connection) => connection,
        Err(error) => {
            tracing::error!("Could not acquire database lock: {error}");
            return render_internal_server_error(Default::default());
        }
    };

    let account = match get_account(account_id, &connection) {
        Ok(transaction) => transaction,
        Err(Error::NotFound) => {
            return get_404_not_found_response();
        }
        Err(error) => {
            tracing::error!("Failed to retrieve transaction {account_id}: {error}");
            return render_internal_server_error(Default::default());
        }
    };

    let Some(local_timezone) = get_local_offset(&state.local_timezone) else {
        tracing::error!("Failed to get local timezone offset");
        return render_internal_server_error(Default::default());
    };

    let edit_url = format_endpoint(endpoints::EDIT_BALANCE, account_id);

    render(
        StatusCode::OK,
        EditAccountPageTemplate {
            nav_bar,
            edit_url,
            max_date: OffsetDateTime::now_utc().to_offset(local_timezone).date(),
            account,
        },
    )
}

/// Retrieve an account from the database by its `id`.
///
/// # Errors
/// This function will return a:
/// - [Error::NotFound] if `id` does not refer to a valid account,
/// - or [Error::SqlError] there is some other SQL error.
fn get_account(id: DatabaseId, connection: &Connection) -> Result<Balance, Error> {
    let account = connection
        .prepare("SELECT id, account, balance, date FROM balance WHERE id = :id")?
        .query_one(&[(":id", &id)], map_row_to_balance)?;

    Ok(account)
}

#[cfg(test)]
mod tests {
    // TODO: test get_account
}
