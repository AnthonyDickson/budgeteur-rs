//! This files defines the routes for the transaction type.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Form, Json,
};
use axum_extra::extract::PrivateCookieJar;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::{
    auth::get_user_id_from_auth_cookie,
    models::{DatabaseID, Transaction, UserID},
    AppError, AppState,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct TransactionData {
    amount: f64,
    // HACK: Date should be a date type. A datetime tpye is used as a workaround since I
    // encountered issues serializing dates with axum_test (this uses serde_urlencoded).
    #[serde(with = "time::serde::iso8601")]
    date: OffsetDateTime,
    description: String,
    category_id: Option<DatabaseID>,
}

/// A route handler for creating a new transaction.
///
/// # Panics
///
/// Panics if the lock for the database connection is already held by the same thread.
pub async fn create_transaction(
    State(state): State<AppState>,
    _jar: PrivateCookieJar,
    Path(user_id): Path<UserID>,
    Form(data): Form<TransactionData>,
) -> impl IntoResponse {
    Transaction::build(data.amount, user_id)
        .description(data.description)
        .category(data.category_id)
        .date(data.date.date())?
        .insert(&state.db_connection().lock().unwrap())
        .map(|transaction| (StatusCode::OK, Json(transaction)))
        .map_err(AppError::TransactionError)
}

/// A route handler for getting a transaction by its database ID.
///
/// This function will return the status code 404 if the requested resource does not exist (e.g., not created yet).
///
/// # Panics
///
/// Panics if the lock for the database connection is already held by the same thread.
pub async fn get_transaction(
    State(state): State<AppState>,
    jar: PrivateCookieJar,
    Path(transaction_id): Path<DatabaseID>,
) -> impl IntoResponse {
    let connection_mutex = state.db_connection();
    let connection = connection_mutex.lock().unwrap();

    Transaction::select(transaction_id, &connection)
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
    use axum_extra::extract::cookie::Cookie;
    use axum_test::TestServer;
    use rusqlite::Connection;
    use time::OffsetDateTime;

    use crate::auth::LogInData;
    use crate::build_router;
    use crate::routes::register::RegisterForm;
    use crate::routes::transaction::TransactionData;
    use crate::routes::CategoryData;
    use crate::{
        auth::COOKIE_USER_ID,
        db::initialize,
        models::{Category, Transaction, UserID},
        routes::endpoints,
        AppState,
    };

    fn get_test_app_config() -> AppState {
        let db_connection =
            Connection::open_in_memory().expect("Could not open database in memory.");
        initialize(&db_connection).expect("Could not initialize database.");

        AppState::new(db_connection, "42".to_string())
    }

    async fn create_app_with_user() -> (TestServer, UserID, Cookie<'static>) {
        let app = build_router(get_test_app_config());

        let server = TestServer::new(app).expect("Could not create test server.");

        let email = "test@test.com".to_string();
        let password = "averysafeandsecurepassword".to_string();

        let response = server
            .post(endpoints::USERS)
            .form(&RegisterForm {
                email: email.clone(),
                password: password.clone(),
                confirm_password: password.clone(),
            })
            .await;

        response.assert_status_see_other();

        let response = server
            .post(endpoints::LOG_IN)
            .form(&LogInData { email, password })
            .await;

        response.assert_status_see_other();
        let auth_cookie = response.cookie(COOKIE_USER_ID);

        // TODO: Implement a way to get the user id from the auth cookie. For now, just guess the user id.
        (server, UserID::new(1), auth_cookie)
    }

    async fn create_app_with_user_and_category() -> (TestServer, UserID, Cookie<'static>, Category)
    {
        let (server, user_id, auth_cookie) = create_app_with_user().await;

        let category = server
            .post(&endpoints::USER_CATEGORIES.replace(":user_id", &user_id.to_string()))
            .add_cookie(auth_cookie.clone())
            .form(&CategoryData {
                name: "foo".to_string(),
            })
            .await
            .json::<Category>();

        (server, user_id, auth_cookie, category)
    }

    #[tokio::test]
    async fn create_transaction() {
        let (server, user_id, auth_cookie, category) = create_app_with_user_and_category().await;

        let amount = -10.0;
        let date = OffsetDateTime::now_utc();
        let description = "A thingymajig";

        let response = server
            .post(&endpoints::USER_TRANSACTIONS.replace(":user_id", &user_id.to_string()))
            .add_cookie(auth_cookie)
            .form(&TransactionData {
                amount,
                date,
                description: description.to_string(),
                category_id: Some(category.id()),
            })
            .await;

        response.assert_status_ok();

        dbg!(response.text());

        let transaction = response.json::<Transaction>();

        assert_eq!(transaction.amount(), amount);
        assert_eq!(*transaction.date(), date.date());
        assert_eq!(transaction.description(), description);
        assert_eq!(transaction.category_id(), Some(category.id()));
        assert_eq!(transaction.user_id(), user_id);
    }

    #[tokio::test]
    async fn get_transaction() {
        let (server, user_id, auth_cookie, category) = create_app_with_user_and_category().await;

        let amount = -10.0;
        let date = OffsetDateTime::now_utc();
        let description = "A thingymajig";

        let inserted_transaction = server
            .post(&endpoints::USER_TRANSACTIONS.replace(":user_id", &user_id.to_string()))
            .add_cookie(auth_cookie.clone())
            .form(&TransactionData {
                amount,
                date,
                description: description.to_string(),
                category_id: Some(category.id()),
            })
            .await
            .json::<Transaction>();

        let response = server
            .get(&format!(
                "{}/{}",
                endpoints::TRANSACTIONS,
                inserted_transaction.id()
            ))
            .add_cookie(auth_cookie)
            .await;

        response.assert_status_ok();

        let selected_transaction = response.json::<Transaction>();

        assert_eq!(selected_transaction, inserted_transaction);
    }

    #[tokio::test]
    async fn get_transaction_fails_on_wrong_user() {
        let (server, user_id, auth_cookie, category) = create_app_with_user_and_category().await;

        let amount = -10.0;
        let date = OffsetDateTime::now_utc();
        let description = "A thingymajig";

        let inserted_transaction = server
            .post(&endpoints::USER_TRANSACTIONS.replace(":user_id", &user_id.to_string()))
            .add_cookie(auth_cookie.clone())
            .form(&TransactionData {
                amount,
                date,
                description: description.to_string(),
                category_id: Some(category.id()),
            })
            .await
            .json::<Transaction>();

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
            .get(&format!("/transaction/{}", inserted_transaction.id()))
            .add_cookie(auth_cookie)
            .await
            .assert_status_not_found();
    }

    // TODO: Add tests for category and transaction that check for correct behaviour when foreign key constraints are violated. Need to also decide what 'correct behaviour' should be.
}
