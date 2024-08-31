use std::time::Duration;

use auth::{get_user_id_from_auth_cookie, AuthError};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use axum_extra::extract::PrivateCookieJar;
use axum_server::Handle;
use db::{Insert, SelectBy};
use model::{
    Category, DatabaseID, NewCategory, NewTransaction, NewUser, PasswordHash, Transaction,
};
use serde_json::json;
use tokio::signal;

pub mod model;

pub use config::AppState;

use crate::{auth::Credentials, db::DbError};

pub mod auth;
mod config;
pub mod db;

/// Return a router with all the app's routes.
pub fn build_router() -> Router<AppState> {
    Router::new()
        .route("/coffee", get(|| async { StatusCode::IM_A_TEAPOT }))
        .route("/user", post(create_user))
        .route("/sign_in", post(auth::sign_in))
        .route("/category", post(create_category))
        .route("/category/:category_id", get(get_category))
        .route("/transaction", post(create_transaction))
        .route("/transaction/:transaction_id", get(get_transaction))
}

/// An async task that waits for either the ctrl+c or terminate signal, whichever comes first, and
/// then signals the server to shut down gracefully.
///
/// `handle` is a handle to an Axum `Server`.
pub async fn graceful_shutdown(handle: Handle) {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            tracing::debug!("Received ctrl+c signal.");
            handle.graceful_shutdown(Some(Duration::from_secs(1)));
        },
        _ = terminate => {
            tracing::debug!("Received terminate signal.");
            handle.graceful_shutdown(Some(Duration::from_secs(1)));
        },
    }
}

enum AppError {
    /// An error occurred in a third-party library.
    InternalError,
    /// An error occurred while creating a user.
    UserCreation(String),
    /// The requested resource was not found. The client should check that the parameters (e.g., ID) are correct and that the resource has been created.
    NotFound,
    /// An error occurred whlie accessing the application's database. This may be due to a database constraint being violated (e.g., foreign keys).
    DatabaseError(DbError),
    /// The user is not authenticated/authorized to access the given resource.
    AuthError(AuthError),
}

impl From<AuthError> for AppError {
    fn from(value: AuthError) -> Self {
        AppError::AuthError(value)
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            AppError::InternalError => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal server error".to_string(),
            ),
            AppError::UserCreation(description) => (StatusCode::OK, description),
            AppError::DatabaseError(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Internal server error: {e:?}"),
            ),
            AppError::AuthError(e) => (StatusCode::UNAUTHORIZED, format!("Auth error: {e:?}")),
            AppError::NotFound => (
                StatusCode::NOT_FOUND,
                "The requested resource could not be found.".to_string(),
            ),
        };

        let body = Json(json!({
            "error": error_message,
        }));

        (status, body).into_response()
    }
}

impl From<DbError> for AppError {
    fn from(e: DbError) -> Self {
        tracing::error!("{e:?}");

        AppError::DatabaseError(e)
    }
}

async fn create_user(
    State(state): State<AppState>,
    Json(user_data): Json<Credentials>,
) -> impl IntoResponse {
    PasswordHash::new(user_data.password)
        .map_err(|e| {
            tracing::error!("Error hashing password: {e:?}");
            AppError::InternalError
        })
        .and_then(|password_hash| {
            NewUser {
                email: user_data.email,
                password_hash,
            }
            .insert(&state.db_connection().lock().unwrap())
            .map(|user| (StatusCode::OK, Json(user)))
            .map_err(|e| AppError::UserCreation(format!("Could not create user: {e:?}")))
        })
}

/// A route handler for creating a new category.
///
/// # Panics
///
/// Panics if the lock for the database connection is already held by the same thread.
async fn create_category(
    State(state): State<AppState>,
    _jar: PrivateCookieJar,
    Json(new_category): Json<NewCategory>,
) -> impl IntoResponse {
    let connection_mutex = state.db_connection();
    let connection = connection_mutex.lock().unwrap();

    new_category
        .insert(&connection)
        .map(|category| (StatusCode::OK, Json(category)))
        .map_err(AppError::DatabaseError)
}

/// A route handler for getting a category by its database ID.
///
/// This function will return the status code 404 if the requested resource does not exist (e.g., not created yet).
///
/// # Panics
///
/// Panics if the lock for the database connection is already held by the same thread.
async fn get_category(
    State(state): State<AppState>,
    jar: PrivateCookieJar,
    Path(category_id): Path<DatabaseID>,
) -> impl IntoResponse {
    let connection_mutex = state.db_connection();
    let connection = connection_mutex.lock().unwrap();

    Category::select(category_id, &connection)
        .map_err(AppError::DatabaseError)
        .and_then(|category| {
            let user_id = get_user_id_from_auth_cookie(jar)?;

            if user_id == category.user_id() {
                Ok(category)
            } else {
                // Respond with 404 not found so that unauthorized users cannot know whether another user's resource exists.
                Err(AppError::NotFound)
            }
        })
        .map(|category| (StatusCode::OK, Json(category)))
}

/// A route handler for creating a new transaction.
///
/// # Panics
///
/// Panics if the lock for the database connection is already held by the same thread.
async fn create_transaction(
    State(state): State<AppState>,
    _jar: PrivateCookieJar,
    Json(new_transaction): Json<NewTransaction>,
) -> impl IntoResponse {
    new_transaction
        .insert(&state.db_connection().lock().unwrap())
        .map(|transaction| (StatusCode::OK, Json(transaction)))
        .map_err(AppError::DatabaseError)
}

/// A route handler for getting a transaction by its database ID.
///
/// This function will return the status code 404 if the requested resource does not exist (e.g., not created yet).
///
/// # Panics
///
/// Panics if the lock for the database connection is already held by the same thread.
async fn get_transaction(
    State(state): State<AppState>,
    jar: PrivateCookieJar,
    Path(transaction_id): Path<DatabaseID>,
) -> impl IntoResponse {
    let connection_mutex = state.db_connection();
    let connection = connection_mutex.lock().unwrap();

    Transaction::select(transaction_id, &connection)
        .map_err(AppError::DatabaseError)
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
mod user_tests {
    use std::str::FromStr;

    use axum::{routing::post, Router};
    use axum_test::TestServer;
    use email_address::EmailAddress;
    use rusqlite::Connection;
    use serde_json::json;

    use crate::{
        create_user,
        db::initialize,
        model::{RawPassword, User},
        AppState,
    };

    fn get_test_app_config() -> AppState {
        let db_connection =
            Connection::open_in_memory().expect("Could not open database in memory.");
        initialize(&db_connection).expect("Could not initialize database.");

        AppState::new(db_connection, "42".to_string())
    }

    #[tokio::test]
    async fn test_create_user() {
        let app = Router::new()
            .route("/user", post(create_user))
            .with_state(get_test_app_config());

        let server = TestServer::new(app).expect("Could not create test server.");

        let email = EmailAddress::from_str("test@test.com").unwrap();
        let password = RawPassword::new_unchecked("hunter2".to_owned());

        let response = server
            .post("/user")
            .content_type("application/json")
            .json(&json!({
                "email": email,
                "password": password
            }))
            .await;

        response.assert_status_ok();

        let user = response.json::<User>();
        assert_eq!(user.email(), &email);
        assert!(user.password_hash().verify(&password).unwrap());
    }
}

#[cfg(test)]
mod category_tests {
    use axum_extra::extract::cookie::Cookie;
    use axum_test::TestServer;
    use rusqlite::Connection;
    use serde_json::json;

    use crate::{
        auth::COOKIE_USER_ID,
        build_router,
        db::initialize,
        model::{Category, CategoryName, User},
        AppState,
    };

    fn get_test_app_config() -> AppState {
        let db_connection =
            Connection::open_in_memory().expect("Could not open database in memory.");
        initialize(&db_connection).expect("Could not initialize database.");

        AppState::new(db_connection, "42".to_string())
    }

    async fn create_app_with_user() -> (TestServer, User, Cookie<'static>) {
        let app = build_router().with_state(get_test_app_config());

        let server = TestServer::new(app).expect("Could not create test server.");

        let email = "test@test.com";
        let password = "hunter2";

        let response = server
            .post("/user")
            .content_type("application/json")
            .json(&json!({
                "email": email,
                "password": password
            }))
            .await;

        response.assert_status_ok();

        let user = response.json::<User>();

        let response = server
            .post("/sign_in")
            .content_type("application/json")
            .json(&json!({
                "email": &user.email(),
                "password": password,
            }))
            .await;

        response.assert_status_ok();
        let auth_cookie = response.cookie(COOKIE_USER_ID);

        (server, user, auth_cookie)
    }

    async fn create_app_with_user_and_category() -> (TestServer, User, Cookie<'static>, Category) {
        let (server, user, auth_cookie) = create_app_with_user().await;

        let category = server
            .post("/category")
            .add_cookie(auth_cookie.clone())
            .content_type("application/json")
            .json(&json!({
                "id": 0,
                "name": "foo",
                "user_id": user.id(),
            }))
            .await
            .json::<Category>();

        (server, user, auth_cookie, category)
    }

    #[tokio::test]
    async fn create_category() {
        let (server, user, auth_cookie) = create_app_with_user().await;

        let name = CategoryName::new("Foo".to_string()).unwrap();

        let response = server
            .post("/category")
            .add_cookie(auth_cookie)
            .content_type("application/json")
            .json(&json!({
                "id": 0,
                "name": name,
                "user_id": user.id(),
            }))
            .await;

        response.assert_status_ok();

        let category = response.json::<Category>();

        assert_eq!(category.name(), &name);
        assert_eq!(category.user_id(), user.id());
    }

    #[tokio::test]
    async fn get_category() {
        let (server, _, auth_cookie, category) = create_app_with_user_and_category().await;

        let response = server
            .get(&format!("/category/{}", category.id()))
            .add_cookie(auth_cookie)
            .await;

        response.assert_status_ok();

        let selected_category = response.json::<Category>();

        assert_eq!(selected_category, category);
    }

    #[tokio::test]
    async fn get_category_fails_on_wrong_user() {
        let (server, _, _, category) = create_app_with_user_and_category().await;

        let email = "test2@test.com";
        let password = "hunter3";

        let user = server
            .post("/user")
            .content_type("application/json")
            .json(&json!({
                "email": email,
                "password": password
            }))
            .await
            .json::<User>();

        let auth_cookie = server
            .post("/sign_in")
            .content_type("application/json")
            .json(&json!({
                "email": user.email(),
                "password": password
            }))
            .await
            .cookie(COOKIE_USER_ID);

        server
            .get(&format!("/category/{}", category.id()))
            .add_cookie(auth_cookie)
            .await
            .assert_status_not_found();
    }
}

#[cfg(test)]
mod transaction_tests {
    use axum_extra::extract::cookie::Cookie;
    use axum_test::TestServer;
    use chrono::Utc;
    use rusqlite::Connection;
    use serde_json::json;

    use crate::{
        auth::COOKIE_USER_ID,
        build_router,
        db::initialize,
        model::{Category, Transaction, User},
        AppState,
    };

    fn get_test_app_config() -> AppState {
        let db_connection =
            Connection::open_in_memory().expect("Could not open database in memory.");
        initialize(&db_connection).expect("Could not initialize database.");

        AppState::new(db_connection, "42".to_string())
    }

    async fn create_app_with_user() -> (TestServer, User, Cookie<'static>) {
        let app = build_router().with_state(get_test_app_config());

        let server = TestServer::new(app).expect("Could not create test server.");

        let email = "test@test.com";
        let password = "hunter2";

        let response = server
            .post("/user")
            .content_type("application/json")
            .json(&json!({
                "email": email,
                "password": password
            }))
            .await;

        response.assert_status_ok();

        let user = response.json::<User>();

        let response = server
            .post("/sign_in")
            .content_type("application/json")
            .json(&json!({
                "email": &user.email(),
                "password": password,
            }))
            .await;

        response.assert_status_ok();
        let auth_cookie = response.cookie(COOKIE_USER_ID);

        (server, user, auth_cookie)
    }

    async fn create_app_with_user_and_category() -> (TestServer, User, Cookie<'static>, Category) {
        let (server, user, auth_cookie) = create_app_with_user().await;

        let category = server
            .post("/category")
            .add_cookie(auth_cookie.clone())
            .content_type("application/json")
            .json(&json!({
                "id": 0,
                "name": "foo",
                "user_id": user.id(),
            }))
            .await
            .json::<Category>();

        (server, user, auth_cookie, category)
    }

    #[tokio::test]
    async fn create_transaction() {
        let (server, user, auth_cookie, category) = create_app_with_user_and_category().await;

        let amount = -10.0;
        let date = Utc::now().date_naive();
        let description = "A thingymajig";

        let response = server
            .post("/transaction")
            .add_cookie(auth_cookie)
            .content_type("application/json")
            .json(&json!({
                "id": 0,
                "amount": amount,
                "date": date,
                "description": description,
                "category_id": category.id(),
                "user_id": user.id(),
            }))
            .await;

        response.assert_status_ok();

        let transaction = response.json::<Transaction>();

        assert_eq!(transaction.amount(), amount);
        assert_eq!(*transaction.date(), date);
        assert_eq!(transaction.description(), description);
        assert_eq!(transaction.category_id(), category.id());
        assert_eq!(transaction.user_id(), user.id());
    }

    #[tokio::test]
    async fn get_transaction() {
        let (server, user, auth_cookie, category) = create_app_with_user_and_category().await;

        let amount = -10.0;
        let date = Utc::now().date_naive();
        let description = "A thingymajig";

        let inserted_transaction = server
            .post("/transaction")
            .add_cookie(auth_cookie.clone())
            .content_type("application/json")
            .json(&json!({
                "id": 0,
                "amount": amount,
                "date": date,
                "description": description,
                "category_id": category.id(),
                "user_id": user.id(),
            }))
            .await
            .json::<Transaction>();

        let response = server
            .get(&format!("/transaction/{}", inserted_transaction.id()))
            .add_cookie(auth_cookie)
            .await;

        response.assert_status_ok();

        let selected_transaction = response.json::<Transaction>();

        assert_eq!(selected_transaction, inserted_transaction);
    }

    #[tokio::test]
    async fn get_transaction_fails_on_wrong_user() {
        let (server, user, auth_cookie, category) = create_app_with_user_and_category().await;

        let amount = -10.0;
        let date = Utc::now().date_naive();
        let description = "A thingymajig";

        let inserted_transaction = server
            .post("/transaction")
            .add_cookie(auth_cookie.clone())
            .content_type("application/json")
            .json(&json!({
                "id": 0,
                "amount": amount,
                "date": date,
                "description": description,
                "category_id": category.id(),
                "user_id": user.id(),
            }))
            .await
            .json::<Transaction>();

        let email = "test2@test.com";
        let password = "hunter3";

        let user = server
            .post("/user")
            .content_type("application/json")
            .json(&json!({
                "email": email,
                "password": password
            }))
            .await
            .json::<User>();

        let auth_cookie = server
            .post("/sign_in")
            .content_type("application/json")
            .json(&json!({
                "email": user.email(),
                "password": password
            }))
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
