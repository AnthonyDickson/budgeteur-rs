//! This files defines the routes for the transaction type.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Form, Json,
};
use axum_extra::extract::PrivateCookieJar;
use serde::Deserialize;
use time::Date;

use crate::{
    auth::get_user_id_from_auth_cookie,
    models::{DatabaseID, Transaction, UserID},
    stores::{CategoryStore, TransactionStore, UserStore},
    AppError, AppState,
};

use super::templates::TransactionRow;

/// The form data for creating a transaction.
#[derive(Debug, Deserialize)]
pub struct TransactionForm {
    /// The value of the transaction in dollars.
    amount: f64,
    /// The date when the transaction ocurred.
    date: Date,
    /// Text detailing the transaction.
    description: String,
    /// The ID of the category to assign the transaction to.
    ///
    /// Zero should be interpreted as `None`.
    category_id: DatabaseID,
}

/// A route handler for creating a new transaction, returns [TransactionRow] as a [Response] on success.
///
/// # Panics
///
/// Panics if the lock for the database connection is already held by the same thread.
pub async fn create_transaction<C, T, U>(
    State(state): State<AppState<C, T, U>>,
    _jar: PrivateCookieJar,
    Path(user_id): Path<UserID>,
    Form(data): Form<TransactionForm>,
) -> impl IntoResponse
where
    C: CategoryStore + Send + Sync,
    T: TransactionStore + Send + Sync,
    U: UserStore + Send + Sync,
{
    // HACK: Zero is used as a sentinel value for None. Currently, options do not work with empty
    // form values. For example, the URL encoded form "num=" will return an error.
    let category = match data.category_id {
        0 => None,
        id => Some(id),
    };

    let transaction = Transaction::build(data.amount, user_id)
        .description(data.description)
        .category(category)
        .date(data.date)?;

    state
        .transaction_store()
        .create_from_builder(transaction)
        .map(|transaction| (StatusCode::OK, TransactionRow { transaction }))
        .map_err(AppError::TransactionError)
}

/// A route handler for getting a transaction by its database ID.
///
/// This function will return the status code 404 if the requested resource does not exist (e.g., not created yet).
///
/// # Panics
///
/// Panics if the lock for the database connection is already held by the same thread.
pub async fn get_transaction<C, T, U>(
    State(state): State<AppState<C, T, U>>,
    jar: PrivateCookieJar,
    Path(transaction_id): Path<DatabaseID>,
) -> impl IntoResponse
where
    C: CategoryStore + Send + Sync,
    T: TransactionStore + Send + Sync,
    U: UserStore + Send + Sync,
{
    state
        .transaction_store()
        .get(transaction_id)
        .map_err(AppError::TransactionError)
        .and_then(|transaction| {
            if get_user_id_from_auth_cookie(jar)? == transaction.user_id() {
                Ok(transaction)
            } else {
                // Respond with 404 not found so that unauthorized users cannot know whether another user's resource exists.
                Err(AppError::NotFound)
            }
        })
        .map(|transaction| (StatusCode::OK, Json(transaction)))
}

#[cfg(test)]
mod transaction_tests {
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    use axum_extra::extract::cookie::Cookie;
    use axum_test::TestServer;
    use rusqlite::Connection;
    use time::{Date, OffsetDateTime};

    use crate::auth::LogInData;
    use crate::build_router;
    use crate::models::{DatabaseID, PasswordHash};
    use crate::routes::category::CategoryData;
    use crate::routes::endpoints::format_endpoint;
    use crate::routes::register::RegisterForm;
    use crate::stores::{SQLiteCategoryStore, SQLiteTransactionStore, SQLiteUserStore, UserStore};
    use crate::{
        auth::COOKIE_USER_ID,
        db::initialize,
        models::{Category, Transaction, UserID},
        routes::endpoints,
        AppState,
    };

    async fn create_app_with_user() -> (TestServer, UserID, Cookie<'static>) {
        let db_connection =
            Connection::open_in_memory().expect("Could not open database in memory.");
        initialize(&db_connection).expect("Could not initialize database.");

        let connection = Arc::new(Mutex::new(db_connection));
        let category_store = SQLiteCategoryStore::new(connection.clone());
        let transaction_store = SQLiteTransactionStore::new(connection.clone());
        let mut user_store = SQLiteUserStore::new(connection.clone());

        let email = "test@test.com".to_string();
        let password = "averysafeandsecurepassword".to_string();
        let user = user_store
            .create(
                email.parse().unwrap(),
                PasswordHash::from_raw_password(password.clone(), 4).unwrap(),
            )
            .unwrap();

        let app = build_router(AppState::new(
            "foobar",
            category_store,
            transaction_store,
            user_store,
        ));

        let server = TestServer::new(app).expect("Could not create test server.");

        let response = server
            .post(endpoints::LOG_IN)
            .form(&LogInData { email, password })
            .await;

        response.assert_status_see_other();
        let auth_cookie = response.cookie(COOKIE_USER_ID);

        (server, user.id(), auth_cookie)
    }

    async fn create_app_with_user_and_category() -> (TestServer, UserID, Cookie<'static>, Category)
    {
        let (server, user_id, auth_cookie) = create_app_with_user().await;

        let category = server
            .post(&format_endpoint(
                endpoints::USER_CATEGORIES,
                user_id.as_i64(),
            ))
            .add_cookie(auth_cookie.clone())
            .form(&CategoryData {
                name: "foo".to_string(),
            })
            .await
            .json::<Category>();

        (server, user_id, auth_cookie, category)
    }

    /// Create a hash map to use as a form for creating a transaction.
    ///
    /// A map of strings is used to avoid errors from trying to serialize `Date` structs in
    /// `TransactionForm`.
    fn transaction_form_as_map(
        amount: f64,
        date: Date,
        description: &str,
        category_id: DatabaseID,
    ) -> HashMap<String, String> {
        let mut form = HashMap::new();

        form.insert(String::from("amount"), amount.to_string());
        form.insert(String::from("date"), date.to_string());
        form.insert(String::from("description"), description.to_string());
        form.insert(String::from("category_id"), category_id.to_string());

        form
    }

    #[tokio::test]
    async fn create_transaction() {
        let (server, user_id, auth_cookie, category) = create_app_with_user_and_category().await;

        let amount = -10.0;
        let date = OffsetDateTime::now_utc().date();
        let description = "A thingymajig";

        let form = transaction_form_as_map(amount, date, description, category.id());

        let response = server
            .post(&format_endpoint(
                endpoints::USER_TRANSACTIONS,
                user_id.as_i64(),
            ))
            .add_cookie(auth_cookie)
            .form(&form)
            .await;

        response.assert_status_ok();

        dbg!(response.text());

        let html_response = response.text();

        assert!(html_response.contains(&amount.to_string()));
        assert!(html_response.contains(&date.to_string()));
        assert!(html_response.contains(description));
        assert!(html_response.contains(&category.id().to_string()));
    }

    #[tokio::test]
    async fn get_transaction() {
        let (server, user_id, auth_cookie, category) = create_app_with_user_and_category().await;

        let amount = -10.0;
        let date = OffsetDateTime::now_utc().date();
        let description = "A thingymajig";

        let form = transaction_form_as_map(amount, date, description, category.id());

        server
            .post(&format_endpoint(
                endpoints::USER_TRANSACTIONS,
                user_id.as_i64(),
            ))
            .add_cookie(auth_cookie.clone())
            .form(&form)
            .await;

        let response = server
            .get(&format_endpoint(
                endpoints::TRANSACTION,
                // Just guess the transaction ID since parsing the HTML response is a PITA.
                1,
            ))
            .add_cookie(auth_cookie)
            .await;

        response.assert_status_ok();

        let selected_transaction = response.json::<Transaction>();

        assert_eq!(amount, selected_transaction.amount());
        assert_eq!(&date, selected_transaction.date());
        assert_eq!(description, selected_transaction.description());
        assert_eq!(Some(category.id()), selected_transaction.category_id());
    }

    #[tokio::test]
    async fn get_transaction_fails_on_wrong_user() {
        let (server, user_id, auth_cookie, category) = create_app_with_user_and_category().await;

        let amount = -10.0;
        let date = OffsetDateTime::now_utc().date();
        let description = "A thingymajig";

        let form = transaction_form_as_map(amount, date, description, category.id());

        server
            .post(&format_endpoint(
                endpoints::USER_TRANSACTIONS,
                user_id.as_i64(),
            ))
            .add_cookie(auth_cookie.clone())
            .form(&form)
            .await;

        let email = "test2@test.com".to_string();
        let password = "averystrongandsecurepassword".to_string();

        let response = server
            .post(endpoints::USERS)
            .form(&RegisterForm {
                email: email.clone(),
                password: password.clone(),
                confirm_password: password.clone(),
            })
            .await;

        response.assert_status_see_other();

        let auth_cookie = server
            .post(endpoints::LOG_IN)
            .form(&LogInData { email, password })
            .await
            .cookie(COOKIE_USER_ID);

        server
            // Just guess the transaction ID since parsing the HTML response is a PITA.
            .get(&format_endpoint(endpoints::TRANSACTION, 1))
            .add_cookie(auth_cookie)
            .await
            .assert_status_not_found();
    }

    // TODO: Add tests for category and transaction that check for correct behaviour when foreign key constraints are violated. Need to also decide what 'correct behaviour' should be.
}
