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
    account::{Account, map_row_to_account},
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
#[template(path = "views/account/edit.html")]
struct EditAccountPageTemplate<'a> {
    nav_bar: NavbarTemplate<'a>,
    edit_url: String,
    max_date: Date,
    account: Account,
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
    let nav_bar = get_nav_bar(endpoints::EDIT_ACCOUNT_VIEW);

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

    let edit_url = format_endpoint(endpoints::EDIT_ACCOUNT, account_id);

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
fn get_account(id: DatabaseId, connection: &Connection) -> Result<Account, Error> {
    let account = connection
        .prepare("SELECT id, name, balance, date FROM account WHERE id = :id")?
        .query_one(&[(":id", &id)], map_row_to_account)?;

    Ok(account)
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;
    use time::macros::date;

    use crate::{
        account::{
            create_endpoint::{AccountForm, create_account},
            edit_page::get_account,
        },
        initialize_db,
    };

    #[test]
    fn test_get_account() {
        let connection = must_create_test_connection();
        let want_account = create_account(
            &AccountForm {
                name: "foo".to_owned(),
                balance: 1.23,
                date: date!(2025 - 11 - 02),
            },
            &connection,
        );

        let got_account = get_account(1, &connection);

        assert_eq!(want_account, got_account);
    }

    #[track_caller]
    fn must_create_test_connection() -> Connection {
        let connection =
            Connection::open_in_memory().expect("could not create in-memory SQLite database");
        initialize_db(&connection).expect("could not initialize test DB");

        connection
    }
}
