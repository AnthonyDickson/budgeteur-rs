//! Defines the endpoint for updating an account
use std::sync::{Arc, Mutex};

use axum::{
    Form,
    extract::{FromRef, Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use axum_htmx::HxRedirect;
use rusqlite::{Connection, params};
use serde::Deserialize;
use time::Date;

use crate::{
    AppState, Error, alert::AlertTemplate, database_id::DatabaseId, endpoints,
    shared_templates::render,
};

/// The state needed to edit an account.
#[derive(Debug, Clone)]
pub struct EditAccountState {
    /// The database connection for managing accounts.
    pub db_connection: Arc<Mutex<Connection>>,
}

impl FromRef<AppState> for EditAccountState {
    fn from_ref(state: &AppState) -> Self {
        Self {
            db_connection: state.db_connection.clone(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct EditAccountForm {
    name: String,
    balance: f64,
    date: Date,
}

pub async fn edit_account_endpoint(
    State(state): State<EditAccountState>,
    Path(account_id): Path<DatabaseId>,
    Form(form): Form<EditAccountForm>,
) -> Response {
    let connection = match state.db_connection.lock() {
        Ok(connection) => connection,
        Err(error) => {
            tracing::error!("Could not aqcuire database lock: {error}");
            return render_error_alert();
        }
    };

    match update_account(account_id, &form, &connection) {
        Ok(0) => {
            tracing::error!(
                "Could not update account {account_id}: update returned zero rows affected"
            );
            return render_error_alert();
        }
        Ok(_) => {}
        Err(error) => {
            tracing::error!("Could not update account{account_id}: {error}");
            return render_error_alert();
        }
    }

    (
        HxRedirect(endpoints::BALANCES.to_owned()),
        StatusCode::SEE_OTHER,
    )
        .into_response()
}

fn render_error_alert() -> Response {
    render(
        StatusCode::INTERNAL_SERVER_ERROR,
        AlertTemplate::error(
            "Could not update account",
            "Try again or check the server logs.",
        ),
    )
}

type RowsAffected = usize;

fn update_account(
    id: DatabaseId,
    account: &EditAccountForm,
    connection: &Connection,
) -> Result<RowsAffected, Error> {
    connection
        .execute(
            "UPDATE balance
        SET \
            account = ?1, \
            balance = ?2, \
            date = ?3 \
        WHERE id = ?4;",
            params![account.name, account.balance, account.date, id,],
        )
        .map_err(Error::from)
}

#[cfg(test)]
mod test {
    use std::sync::{Arc, Mutex};

    use axum::{
        Form,
        extract::{Path, State},
        http::{HeaderValue, StatusCode},
    };
    use axum_htmx::HX_REDIRECT;
    use rusqlite::{Connection, params};
    use time::macros::date;

    use crate::{
        balance::{
            Balance,
            create_endpoint::{AccountBalanceForm, create_account_balance},
            edit_account_endpoint,
            edit_endpoint::{EditAccountForm, EditAccountState},
            map_row_to_balance,
        },
        database_id::DatabaseId,
        endpoints, initialize_db,
    };

    #[tokio::test]
    async fn can_update_transaction() {
        let conn = must_create_test_connection();
        let form = EditAccountForm {
            name: "test".to_owned(),
            balance: 1.23,
            date: date!(2025 - 11 - 02),
        };
        let want_account = create_account_balance(
            &AccountBalanceForm {
                name: form.name.clone(),
                balance: form.balance,
                date: form.date,
            },
            &conn,
        )
        .expect("could not create test account");
        let state = EditAccountState {
            db_connection: Arc::new(Mutex::new(conn)),
        };

        let response =
            edit_account_endpoint(State(state.clone()), Path(want_account.id), Form(form)).await;

        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        assert_eq!(
            response.headers().get(HX_REDIRECT),
            Some(&HeaderValue::from_str(endpoints::BALANCES).unwrap())
        );
        let got_account = must_get_account(
            want_account.id,
            &state.db_connection.lock().expect("could not fetch string"),
        );
        assert_eq!(want_account, got_account);
    }

    #[track_caller]
    fn must_create_test_connection() -> Connection {
        let connection =
            Connection::open_in_memory().expect("could not create in-memory SQLite database");
        initialize_db(&connection).expect("could not initialize test DB");

        connection
    }

    #[track_caller]
    fn must_get_account(account_id: DatabaseId, connection: &Connection) -> Balance {
        connection
            .query_one(
                "SELECT id, account, balance, date FROM balance WHERE id = ?1",
                params![account_id],
                map_row_to_balance,
            )
            .unwrap()
    }
}
