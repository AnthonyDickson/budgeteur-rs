//! Defines the endpoint for creating a new account.
use std::sync::{Arc, Mutex};

use axum::{
    Form,
    extract::{FromRef, State},
    http::StatusCode,
    response::IntoResponse,
};
use axum_htmx::HxRedirect;
use rusqlite::{Connection, params};
use serde::Deserialize;
use time::Date;

use crate::{
    AppState, Error, account::Account, alert::AlertTemplate, endpoints, shared_templates::render,
};

/// The state needed to get or create an account.
#[derive(Debug, Clone)]
pub struct CreateAccountState {
    /// The database connection for managing accounts.
    pub db_connection: Arc<Mutex<Connection>>,
}

impl FromRef<AppState> for CreateAccountState {
    fn from_ref(state: &AppState) -> Self {
        Self {
            db_connection: state.db_connection.clone(),
        }
    }
}

/// The form data for creating an account.
#[derive(Debug, Deserialize)]
pub struct AccountForm {
    /// The account name (number)
    pub name: String,
    /// The balance in dollars.
    pub balance: f64,
    /// The date when the account was last checked/updated.
    pub date: Date,
}

/// A route handler for creating a new account, redirects to accounts view on success.
pub async fn create_account_endpoint(
    State(state): State<CreateAccountState>,
    Form(form): Form<AccountForm>,
) -> impl IntoResponse {
    let connection = match state.db_connection.lock() {
        Ok(connection) => connection,
        Err(error) => {
            tracing::error!("Could not acquire database lock: {error}");
            return render(
                StatusCode::INTERNAL_SERVER_ERROR,
                AlertTemplate::error(
                    "Something went wrong",
                    "Try again later or check the server logs",
                ),
            );
        }
    };

    match create_account(&form, &connection) {
        Ok(_) => {}
        Err(Error::DuplicateAccountName) => {
            return render(
                StatusCode::INTERNAL_SERVER_ERROR,
                AlertTemplate::error(
                    "Duplicate Account Name",
                    &format!(
                        "The account {} already exists in the database. \
                        Choose a different account name, or edit or delete the existing account.",
                        form.name
                    ),
                ),
            );
        }
        Err(error) => {
            tracing::error!(
                "Could not create account with {form:?}, got an unexpected error: {error}"
            );
            return render(
                StatusCode::INTERNAL_SERVER_ERROR,
                AlertTemplate::error(
                    "Something went wrong",
                    "Try again later or check the server logs",
                ),
            );
        }
    }

    (
        HxRedirect(endpoints::ACCOUNTS.to_owned()),
        StatusCode::SEE_OTHER,
    )
        .into_response()
}

pub fn create_account(form: &AccountForm, connection: &Connection) -> Result<Account, Error> {
    connection
        .execute(
            "INSERT INTO account (name, balance, date) VALUES (?1, ?2, ?3)",
            params![form.name, form.balance, form.date],
        )
        .map_err(|error| match error {
            // Handle unique account name constraint violation
            rusqlite::Error::SqliteFailure(error, Some(_)) if error.extended_code == 2067 => {
                Error::DuplicateAccountName
            }
            error => error.into(),
        })?;

    let id = connection.last_insert_rowid();

    Ok(Account {
        id,
        name: form.name.clone(),
        balance: form.balance,
        date: form.date,
    })
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use axum::{Form, body::Body, extract::State, http::Response, response::IntoResponse};
    use axum_htmx::HX_REDIRECT;
    use rusqlite::{Connection, params};
    use time::OffsetDateTime;

    use crate::{
        account::{
            Account, create_account_endpoint,
            create_endpoint::{AccountForm, CreateAccountState},
            map_row_to_account,
        },
        database_id::DatabaseId,
        db::initialize,
        endpoints,
    };

    fn get_test_connection() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();
        conn
    }

    #[tokio::test]
    async fn can_create_account() {
        let conn = get_test_connection();
        let state = CreateAccountState {
            db_connection: Arc::new(Mutex::new(conn)),
        };
        let want_account = Account {
            id: 1,
            name: "test account".to_owned(),
            balance: 123.45,
            date: OffsetDateTime::now_utc().date(),
        };

        let form = AccountForm {
            name: want_account.name.clone(),
            balance: want_account.balance,
            date: want_account.date,
        };

        let response = create_account_endpoint(State(state.clone()), Form(form))
            .await
            .into_response();

        assert_redirects_to_accounts_view(response);

        // Verify the account was actually created by getting it by ID
        // We know the first account will have ID 1
        let connection = state.db_connection.lock().unwrap();
        let got_account = must_get_account(1, &connection);
        assert_eq!(want_account, got_account);
    }

    #[track_caller]
    fn must_get_account(id: DatabaseId, connection: &Connection) -> Account {
        connection
            .query_one(
                "SELECT id, name, balance, date FROM account WHERE id = ?1",
                params![id],
                map_row_to_account,
            )
            .expect("could not get account from database")
    }

    #[track_caller]
    fn assert_redirects_to_accounts_view(response: Response<Body>) {
        let location = response
            .headers()
            .get(HX_REDIRECT)
            .expect("expected response to have the header hx-redirect");
        assert_eq!(
            location,
            endpoints::ACCOUNTS,
            "got redirect to {location:?}, want redirect to {}",
            endpoints::ACCOUNTS
        );
    }
}
