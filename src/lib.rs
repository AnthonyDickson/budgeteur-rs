use std::time::Duration;

use askama::Template;
use auth::{auth_guard, get_user_id_from_auth_cookie, AuthError};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    middleware::{self},
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
    Json, Router,
};
use axum_extra::{extract::PrivateCookieJar, response::Html};
use axum_server::Handle;
use db::{Insert, SelectBy};
use model::{
    Category, DatabaseID, NewCategory, NewTransaction, NewUser, PasswordHash, Transaction, UserID,
};
use serde_json::json;
use tokio::signal;

pub mod model;

pub use config::AppState;

use crate::{auth::Credentials, db::DbError};

pub mod auth;
mod config;
pub mod db;

/// The API endpoints.
mod routes {
    pub const COFFEE: &str = "/coffee";
    pub const DASHBOARD: &str = "/dashboard";
    pub const ROOT: &str = "/";
    pub const SIGN_IN: &str = "/sign_in";
    pub const USERS: &str = "/users";
    pub const CATEGORIES: &str = "/categories";
    pub const CATEGORY: &str = "/categories/:category_id";
    pub const TRANSACTIONS: &str = "/transactions";
    pub const TRANSACTION: &str = "/transactions/:transaction_id";
}

// TODO: Update existing routes to respond with HTML
/// Return a router with all the app's routes.
pub fn build_router(state: AppState) -> Router {
    let unprotected_routes = Router::new()
        .route(routes::COFFEE, get(get_coffee))
        .route(routes::SIGN_IN, get(get_sign_in_page))
        .route(routes::SIGN_IN, post(auth::sign_in))
        .route(routes::USERS, post(create_user));

    let protected_routes = Router::new()
        .route(routes::ROOT, get(get_index_page))
        .route(routes::DASHBOARD, get(get_dashboard_page))
        .route(routes::CATEGORIES, post(create_category))
        .route(routes::CATEGORY, get(get_category))
        .route(routes::TRANSACTIONS, post(create_transaction))
        .route(routes::TRANSACTION, get(get_transaction))
        .layer(middleware::from_fn_with_state(state.clone(), auth_guard));

    protected_routes.merge(unprotected_routes).with_state(state)
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

/// Attempt to get a cup of coffee from the server.
async fn get_coffee() -> Response {
    (StatusCode::IM_A_TEAPOT, Html("I'm a teapot")).into_response()
}

/// The root path '/' redirects to the dashboard page.
async fn get_index_page() -> Redirect {
    Redirect::to(routes::DASHBOARD)
}

const INTERNAL_SERVER_ERROR_HTML: &str = "
    <!doctype html>
    <html lang=\"en\">
        <head>
            <title>500 Internal Server Error</title>
        </head>
        <body>
        <h1>Internal Server Error</h1>
        <p>The server was unable to complete your request. Please try again later.</p>
        </body>
    </html>
";

// TODO: Replace below function with implementation of IntoResponse on Template newtype. Refer to https://github.com/tokio-rs/axum/blob/ef8a9e812c1b49b61d21813cb30f5982d8da56df/examples/templates/src/main.rs#L49C1-L65C2
/// Converts the result of an askama template render into a response.
///
/// If the template rendered successfully, the status code 200 OK is return along with the rendered HTML.
/// Otherwise, the status code 500 INTERAL SERVER ERROR is returned along with a static error page.
fn render_result_or_error(template_result: Result<String, askama::Error>) -> Response {
    match template_result {
        Ok(html) => (StatusCode::OK, Html(html)),
        Err(e) => {
            tracing::error!("Error rendering template: {}", e);

            (
                StatusCode::INTERNAL_SERVER_ERROR,
                // Use a static string to avoid any other errors.
                Html(INTERNAL_SERVER_ERROR_HTML.to_string()),
            )
        }
    }
    .into_response()
}

#[derive(Template)]
#[template(path = "views/dashboard.html")]
struct DashboardTemplate {
    user_id: UserID,
}

/// Display a page with an overview of the users data.
async fn get_dashboard_page(jar: PrivateCookieJar) -> Response {
    // TODO: Pass user_id from `auth_guard` middleware using extensions: https://docs.rs/axum/latest/axum/middleware/index.html#passing-state-from-middleware-to-handlers
    match get_user_id_from_auth_cookie(jar.clone()) {
        Ok(user_id) => {
            let tempalte = DashboardTemplate { user_id };
            // TODO: How to handle template render error?
            let rendered_html = tempalte.render();

            render_result_or_error(rendered_html)
        }
        Err(_) => Redirect::to(routes::SIGN_IN).into_response(),
    }
}

#[derive(Template)]
#[template(path = "views/sign_in.html")]
struct SignInTemplate;

/// Display the sign-in page.
async fn get_sign_in_page() -> Response {
    render_result_or_error(SignInTemplate.render())
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
mod root_route_tests {
    use axum::{http::StatusCode, middleware, routing::get, Router};
    use axum_test::TestServer;
    use email_address::EmailAddress;
    use rusqlite::Connection;

    use crate::{
        auth::auth_guard,
        db::{initialize, Insert},
        get_index_page,
        model::{NewUser, PasswordHash, RawPassword},
        routes, AppState,
    };

    fn get_test_app_config() -> AppState {
        let db_connection =
            Connection::open_in_memory().expect("Could not open database in memory.");
        initialize(&db_connection).expect("Could not initialize database.");

        NewUser {
            email: EmailAddress::new_unchecked("test@test.com"),
            password_hash: PasswordHash::new(RawPassword::new_unchecked("test".to_string()))
                .unwrap(),
        }
        .insert(&db_connection)
        .unwrap();

        AppState::new(db_connection, "42".to_string())
    }

    #[tokio::test]
    async fn root_redirects_to_dashbord() {
        let app_state = get_test_app_config();
        let app = Router::new()
            .route(routes::ROOT, get(get_index_page))
            .layer(middleware::from_fn_with_state(
                app_state.clone(),
                auth_guard,
            ))
            .route(routes::SIGN_IN, get(get_index_page))
            .with_state(app_state);
        let server = TestServer::new(app).expect("Could not create test server.");

        let response = server.get(routes::ROOT).await;

        response.assert_status(StatusCode::SEE_OTHER);
        assert_eq!(response.header("location"), routes::SIGN_IN);
    }
}

#[cfg(test)]
mod dashboard_route_tests {
    use axum::{
        http::StatusCode,
        middleware,
        routing::{get, post},
        Router,
    };
    use axum_extra::extract::cookie::Cookie;
    use axum_test::TestServer;
    use email_address::EmailAddress;
    use rusqlite::Connection;
    use serde_json::json;
    use time::{Duration, OffsetDateTime};

    use crate::{
        auth::{sign_in, COOKIE_USER_ID},
        auth_guard,
        db::{initialize, Insert},
        get_dashboard_page,
        model::{NewUser, PasswordHash, RawPassword},
        routes, AppState,
    };

    fn get_test_server() -> TestServer {
        let db_connection =
            Connection::open_in_memory().expect("Could not open database in memory.");
        initialize(&db_connection).expect("Could not initialize database.");

        NewUser {
            email: EmailAddress::new_unchecked("test@test.com"),
            password_hash: PasswordHash::new(RawPassword::new_unchecked("test".to_string()))
                .unwrap(),
        }
        .insert(&db_connection)
        .unwrap();

        let state = AppState::new(db_connection, "42".to_string());
        let app = Router::new()
            .route(routes::DASHBOARD, get(get_dashboard_page))
            .layer(middleware::from_fn_with_state(state.clone(), auth_guard))
            .route(routes::SIGN_IN, post(sign_in))
            .with_state(state);

        TestServer::new(app).expect("Could not create test server.")
    }

    #[tokio::test]
    async fn dashboard_redirects_to_sign_in_without_auth_cookie() {
        let server = get_test_server();

        let response = server.get(routes::DASHBOARD).await;

        response.assert_status(StatusCode::SEE_OTHER);
        assert_eq!(response.header("location"), routes::SIGN_IN);
    }

    #[tokio::test]
    async fn dashboard_redirects_to_sign_in_with_invalid_auth_cookie() {
        let server = get_test_server();

        let fake_auth_cookie = Cookie::build((COOKIE_USER_ID, "1"))
            .secure(true)
            .http_only(true)
            .same_site(axum_extra::extract::cookie::SameSite::Lax)
            .build();
        let response = server
            .get(routes::DASHBOARD)
            .add_cookie(fake_auth_cookie)
            .await;

        response.assert_status(StatusCode::SEE_OTHER);
        assert_eq!(response.header("location"), routes::SIGN_IN);
    }

    #[tokio::test]
    async fn dashboard_redirects_to_sign_in_with_expired_auth_cookie() {
        let server = get_test_server();
        let mut expired_auth_cookie = server
            .post(routes::SIGN_IN)
            .json(&json!({
                "email": "test@test.com",
                "password": "test"
            }))
            .await
            .cookie(COOKIE_USER_ID);

        expired_auth_cookie.set_expires(OffsetDateTime::now_utc() - Duration::weeks(1));

        let response = server
            .get(routes::DASHBOARD)
            .add_cookie(expired_auth_cookie)
            .await;

        response.assert_status(StatusCode::SEE_OTHER);
        assert_eq!(response.header("location"), routes::SIGN_IN);
    }

    #[tokio::test]
    async fn dashboard_displays_with_auth_cookie() {
        let server = get_test_server();

        let auth_cookie = server
            .post(routes::SIGN_IN)
            .json(&json!({
                "email": "test@test.com",
                "password": "test"
            }))
            .await
            .cookie(COOKIE_USER_ID);

        server
            .get(routes::DASHBOARD)
            .add_cookie(auth_cookie)
            .await
            .assert_status_ok();
    }
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
        routes, AppState,
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
            .route(routes::USERS, post(create_user))
            .with_state(get_test_app_config());

        let server = TestServer::new(app).expect("Could not create test server.");

        let email = EmailAddress::from_str("test@test.com").unwrap();
        let password = RawPassword::new_unchecked("hunter2".to_owned());

        let response = server
            .post(routes::USERS)
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
        routes, AppState,
    };

    fn get_test_app_config() -> AppState {
        let db_connection =
            Connection::open_in_memory().expect("Could not open database in memory.");
        initialize(&db_connection).expect("Could not initialize database.");

        AppState::new(db_connection, "42".to_string())
    }

    async fn create_app_with_user() -> (TestServer, User, Cookie<'static>) {
        let app = build_router(get_test_app_config());

        let server = TestServer::new(app).expect("Could not create test server.");

        let email = "test@test.com";
        let password = "hunter2";

        let response = server
            .post(routes::USERS)
            .content_type("application/json")
            .json(&json!({
                "email": email,
                "password": password
            }))
            .await;

        response.assert_status_ok();

        let user = response.json::<User>();

        let response = server
            .post(routes::SIGN_IN)
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
            .post(routes::CATEGORIES)
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
            .post(routes::CATEGORIES)
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
            .get(&format!("{}/{}", routes::CATEGORIES, category.id()))
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
            .post(routes::USERS)
            .content_type("application/json")
            .json(&json!({
                "email": email,
                "password": password
            }))
            .await
            .json::<User>();

        let auth_cookie = server
            .post(routes::SIGN_IN)
            .content_type("application/json")
            .json(&json!({
                "email": user.email(),
                "password": password
            }))
            .await
            .cookie(COOKIE_USER_ID);

        server
            .get(&format!("{}/{}", routes::CATEGORIES, category.id()))
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
        routes, AppState,
    };

    fn get_test_app_config() -> AppState {
        let db_connection =
            Connection::open_in_memory().expect("Could not open database in memory.");
        initialize(&db_connection).expect("Could not initialize database.");

        AppState::new(db_connection, "42".to_string())
    }

    async fn create_app_with_user() -> (TestServer, User, Cookie<'static>) {
        let app = build_router(get_test_app_config());

        let server = TestServer::new(app).expect("Could not create test server.");

        let email = "test@test.com";
        let password = "hunter2";

        let response = server
            .post(routes::USERS)
            .content_type("application/json")
            .json(&json!({
                "email": email,
                "password": password
            }))
            .await;

        response.assert_status_ok();

        let user = response.json::<User>();

        let response = server
            .post(routes::SIGN_IN)
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
            .post(routes::CATEGORIES)
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
            .post(routes::TRANSACTIONS)
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
            .post(routes::TRANSACTIONS)
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
            .get(&format!(
                "{}/{}",
                routes::TRANSACTIONS,
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
        let (server, user, auth_cookie, category) = create_app_with_user_and_category().await;

        let amount = -10.0;
        let date = Utc::now().date_naive();
        let description = "A thingymajig";

        let inserted_transaction = server
            .post(routes::TRANSACTIONS)
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
            .post(routes::USERS)
            .content_type("application/json")
            .json(&json!({
                "email": email,
                "password": password
            }))
            .await
            .json::<User>();

        let auth_cookie = server
            .post(routes::SIGN_IN)
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
