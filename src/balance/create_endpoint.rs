//! Defines the endpoint for creating a new account balance.
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
    AppState, Error, alert::AlertTemplate, balance::Balance, endpoints, shared_templates::render,
};

/// The state needed to get or create an account balance.
#[derive(Debug, Clone)]
pub struct CreateBalanceState {
    /// The database connection for managing account balances.
    pub db_connection: Arc<Mutex<Connection>>,
}

impl FromRef<AppState> for CreateBalanceState {
    fn from_ref(state: &AppState) -> Self {
        Self {
            db_connection: state.db_connection.clone(),
        }
    }
}

/// The form data for creating an account balance.
#[derive(Debug, Deserialize)]
pub struct AccountBalanceForm {
    /// The account name (number)
    pub name: String,
    /// The balance in dollars.
    pub balance: f64,
    /// The date when the balance was last checked/updated.
    pub date: Date,
}

/// A route handler for creating a new account balance, redirects to balances view on success.
pub async fn create_account_balance_endpoint(
    State(state): State<CreateBalanceState>,
    Form(form): Form<AccountBalanceForm>,
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

    match create_account_balance(&form, &connection) {
        Ok(_) => {}
        Err(Error::DuplicateAccountName) => {
            return render(
                StatusCode::INTERNAL_SERVER_ERROR,
                AlertTemplate::error(
                    "Duplicate Account Name",
                    &format!(
                        "The account {} already exists in the database. \
                        Choose a different account name, or edit or delete the existing account balance.",
                        form.name
                    ),
                ),
            );
        }
        Err(error) => {
            tracing::error!(
                "Could not create account balance with {form:?}, got an unexpected error: {error}"
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
        HxRedirect(endpoints::BALANCES.to_owned()),
        StatusCode::SEE_OTHER,
    )
        .into_response()
}

fn create_account_balance(
    form: &AccountBalanceForm,
    connection: &Connection,
) -> Result<Balance, Error> {
    connection
        .execute(
            "INSERT INTO balance (account, balance, date) VALUES (?1, ?2, ?3)",
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

    Ok(Balance {
        id,
        account: form.name.clone(),
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
        balance::{
            Balance, create_account_balance_endpoint,
            create_endpoint::{AccountBalanceForm, CreateBalanceState},
            map_row_to_balance,
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
    async fn can_create_balance() {
        let conn = get_test_connection();
        let state = CreateBalanceState {
            db_connection: Arc::new(Mutex::new(conn)),
        };
        let want_balance = Balance {
            id: 1,
            account: "test account".to_owned(),
            balance: 123.45,
            date: OffsetDateTime::now_utc().date(),
        };

        let form = AccountBalanceForm {
            name: want_balance.account.clone(),
            balance: want_balance.balance,
            date: want_balance.date,
        };

        let response = create_account_balance_endpoint(State(state.clone()), Form(form))
            .await
            .into_response();

        assert_redirects_to_balances_view(response);

        // Verify the transaction was actually created by getting it by ID
        // We know the first transaction will have ID 1
        let connection = state.db_connection.lock().unwrap();
        let got_balance = must_get_balance(1, &connection);
        assert_eq!(want_balance, got_balance);
    }

    #[track_caller]
    fn must_get_balance(id: DatabaseId, connection: &Connection) -> Balance {
        connection
            .query_one(
                "SELECT id, account, balance, date FROM balance WHERE id = ?1",
                params![id],
                map_row_to_balance,
            )
            .expect("could not get balance from database")
    }

    #[track_caller]
    fn assert_redirects_to_balances_view(response: Response<Body>) {
        let location = response
            .headers()
            .get(HX_REDIRECT)
            .expect("expected response to have the header hx-redirect");
        assert_eq!(
            location,
            endpoints::BALANCES,
            "got redirect to {location:?}, want redirect to {}",
            endpoints::BALANCES
        );
    }
}
