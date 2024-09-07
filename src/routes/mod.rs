use askama::Template;
use axum::{
    extract::{Path, State},
    http::{StatusCode, Uri},
    middleware,
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
    Extension, Form, Json, Router,
};
use axum_extra::extract::PrivateCookieJar;
use axum_htmx::HxRedirect;

use register::{create_user, get_register_page, EmailInputTemplate, PasswordInputTemplate};

use crate::{
    auth::{
        auth_guard, get_user_id_from_auth_cookie, set_auth_cookie, verify_credentials, AuthError,
        LogInData,
    },
    db::{Insert, SelectBy},
    models::{Category, DatabaseID, NewCategory, NewTransaction, Transaction, UserID},
    AppError, AppState, HtmlTemplate,
};

pub mod register;

/// The API endpoints URIs.
pub mod endpoints {
    pub const COFFEE: &str = "/coffee";
    pub const DASHBOARD: &str = "/dashboard";
    pub const ROOT: &str = "/";
    pub const LOG_IN: &str = "/log_in";
    pub const REGISTER: &str = "/register";
    pub const USERS: &str = "/users";
    pub const CATEGORIES: &str = "/categories";
    pub const CATEGORY: &str = "/categories/:category_id";
    pub const TRANSACTIONS: &str = "/transactions";
    pub const TRANSACTION: &str = "/transactions/:transaction_id";
    pub const INTERNAL_ERROR: &str = "/error";
}

// TODO: Update existing routes to respond with HTML
/// Return a router with all the app's routes.
pub fn build_router(state: AppState) -> Router {
    let unprotected_routes = Router::new()
        .route(endpoints::COFFEE, get(get_coffee))
        .route(endpoints::LOG_IN, get(get_log_in_page))
        .route(endpoints::LOG_IN, post(log_in))
        .route(endpoints::REGISTER, get(get_register_page))
        .route(endpoints::USERS, post(create_user))
        .route(
            endpoints::INTERNAL_ERROR,
            get(get_internal_server_error_page),
        );

    let protected_routes = Router::new()
        .route(endpoints::ROOT, get(get_index_page))
        .route(endpoints::DASHBOARD, get(get_dashboard_page))
        .route(endpoints::CATEGORIES, post(create_category))
        .route(endpoints::CATEGORY, get(get_category))
        .route(endpoints::TRANSACTIONS, post(create_transaction))
        .route(endpoints::TRANSACTION, get(get_transaction))
        .layer(middleware::from_fn_with_state(state.clone(), auth_guard));

    protected_routes
        .merge(unprotected_routes)
        .fallback(get_404_not_found)
        .with_state(state)
}

/// Attempt to get a cup of coffee from the server.
async fn get_coffee() -> Response {
    (StatusCode::IM_A_TEAPOT, Html("I'm a teapot")).into_response()
}

/// The root path '/' redirects to the dashboard page.
async fn get_index_page() -> Redirect {
    Redirect::to(endpoints::DASHBOARD)
}

/// Get a response that will redirect the client to the internal server error 500 page.
///
/// **Note**: This redirect is intended to be served as a response to a POST request initiated by HTMX.
/// Route handlers using GET should use `axum::response::Redirect`.
pub(crate) fn get_internal_server_error_redirect() -> Response {
    (
        HxRedirect(Uri::from_static(endpoints::INTERNAL_ERROR)),
        StatusCode::INTERNAL_SERVER_ERROR,
    )
        .into_response()
}

#[derive(Template)]
#[template(path = "views/internal_server_error_500.html")]
struct InternalServerErrorPageTemplate;

async fn get_internal_server_error_page() -> Response {
    HtmlTemplate(InternalServerErrorPageTemplate).into_response()
}

#[derive(Template)]
#[template(path = "views/not_found_404.html")]
struct NotFoundTemplate;

async fn get_404_not_found() -> Response {
    (StatusCode::NOT_FOUND, HtmlTemplate(NotFoundTemplate)).into_response()
}

#[derive(Template)]
#[template(path = "views/dashboard.html")]
struct DashboardTemplate {
    user_id: UserID,
}

/// Display a page with an overview of the user's data.
async fn get_dashboard_page(Extension(user_id): Extension<UserID>) -> Response {
    HtmlTemplate(DashboardTemplate { user_id }).into_response()
}

#[derive(Template)]
#[template(path = "partials/log_in/form.html")]
struct LogInFormTemplate<'a> {
    email_input: EmailInputTemplate<'a>,
    password_input: PasswordInputTemplate<'a>,
    log_in_route: &'a str,
    register_route: &'a str,
}

impl Default for LogInFormTemplate<'_> {
    fn default() -> Self {
        Self {
            email_input: Default::default(),
            password_input: Default::default(),
            log_in_route: endpoints::LOG_IN,
            register_route: endpoints::REGISTER,
        }
    }
}

#[derive(Template, Default)]
#[template(path = "views/log_in.html")]
struct LogInTemplate<'a> {
    log_in_form: LogInFormTemplate<'a>,
}

/// Display the log-in page.
async fn get_log_in_page() -> Response {
    HtmlTemplate(LogInTemplate::default()).into_response()
}

/// Handler for log-in requests via the POST method.
///
/// On a successful log-in request, the auth cookie set and the client is redirected to the dashboard page.
/// Otherwise, the form is return with an error message explaining the problem.
///
/// # Errors
///
/// This function will return an error in a few situations.
/// - The email does not belong to a registered user.
/// - The password is not correct.
/// - An internal error occurred when verifying the password.
async fn log_in(
    State(state): State<AppState>,
    jar: PrivateCookieJar,
    Form(user_data): Form<LogInData>,
) -> Response {
    verify_credentials(user_data.clone(), &state.db_connection().lock().unwrap())
        .map(|user| {
            let jar = set_auth_cookie(jar, user.id());

            (
                StatusCode::SEE_OTHER,
                HxRedirect(Uri::from_static(endpoints::DASHBOARD)),
                jar,
            )
        })
        .map_err(|e| {
            HtmlTemplate(LogInFormTemplate {
                email_input: EmailInputTemplate {
                    value: &user_data.email,
                    error_message: "",
                },
                password_input: PasswordInputTemplate {
                    value: "",
                    min_length: 0,
                    error_message: match e {
                        AuthError::InvalidCredentials => "Incorrect email or password.",
                        AuthError::InternalError => {
                            "An internal error occurred. Please try again later."
                        }
                    },
                },
                ..Default::default()
            })
        })
        .into_response()
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
mod log_in_tests {
    use axum::{http::StatusCode, routing::post, Router};
    use axum_test::TestServer;
    use email_address::EmailAddress;
    use rusqlite::Connection;

    use crate::{
        auth::LogInData,
        db::{initialize, Insert},
        models::{NewUser, PasswordHash, ValidatedPassword},
        routes::{endpoints, log_in},
        AppState,
    };

    fn get_test_app_config() -> AppState {
        let db_connection =
            Connection::open_in_memory().expect("Could not open database in memory.");
        initialize(&db_connection).expect("Could not initialize database.");

        NewUser {
            email: EmailAddress::new_unchecked("test@test.com"),
            password_hash: PasswordHash::new(ValidatedPassword::new_unchecked("test".to_string()))
                .unwrap(),
        }
        .insert(&db_connection)
        .unwrap();

        AppState::new(db_connection, "42".to_string())
    }

    #[tokio::test]
    async fn log_in_succeeds_with_valid_credentials() {
        let app = Router::new()
            .route(endpoints::LOG_IN, post(log_in))
            .with_state(get_test_app_config());

        let server = TestServer::new(app).expect("Could not create test server.");

        server
            .post(endpoints::LOG_IN)
            .form(&LogInData {
                email: "test@test.com".to_string(),
                password: "test".to_string(),
            })
            .await
            .assert_status(StatusCode::SEE_OTHER);
    }

    #[tokio::test]
    async fn log_in_fails_with_missing_credentials() {
        let app = Router::new()
            .route(endpoints::LOG_IN, post(log_in))
            .with_state(get_test_app_config());

        let server = TestServer::new(app).expect("Could not create test server.");

        server
            .post(endpoints::LOG_IN)
            .content_type("application/x-www-form-urlencoded")
            .await
            .assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn log_in_fails_with_incorrect_email() {
        let app = Router::new()
            .route(endpoints::LOG_IN, post(log_in))
            .with_state(get_test_app_config());

        let server = TestServer::new(app).expect("Could not create test server.");

        server
            .post(endpoints::LOG_IN)
            .form(&LogInData {
                email: "wrong@email.com".to_string(),
                password: "test".to_string(),
            })
            .await
            .text()
            .contains("invalid");
    }

    #[tokio::test]
    async fn log_in_fails_with_incorrect_password() {
        let app = Router::new()
            .route(endpoints::LOG_IN, post(log_in))
            .with_state(get_test_app_config());

        let server = TestServer::new(app).expect("Could not create test server.");

        server
            .post(endpoints::LOG_IN)
            .form(&LogInData {
                email: "test@test.com".to_string(),
                password: "wrongpassword".to_string(),
            })
            .await
            .text()
            .contains("invalid");
    }
}

// These tests are here so that we know when we call `Uri::from_shared` it will not panic.
#[cfg(test)]
mod endpoints_tests {
    use axum::http::Uri;

    use crate::routes::endpoints;

    fn assert_endpoint_is_valid_uri(uri: &str) {
        assert!(uri.parse::<Uri>().is_ok());
    }

    #[test]
    fn endpoints_are_valid_uris() {
        assert_endpoint_is_valid_uri(endpoints::CATEGORIES);
        assert_endpoint_is_valid_uri(endpoints::CATEGORY);
        assert_endpoint_is_valid_uri(endpoints::COFFEE);
        assert_endpoint_is_valid_uri(endpoints::DASHBOARD);
        assert_endpoint_is_valid_uri(endpoints::LOG_IN);
        assert_endpoint_is_valid_uri(endpoints::REGISTER);
        assert_endpoint_is_valid_uri(endpoints::ROOT);
        assert_endpoint_is_valid_uri(endpoints::USERS);
    }
}

#[cfg(test)]
mod root_route_tests {
    use axum::{middleware, routing::get, Router};
    use axum_test::TestServer;
    use email_address::EmailAddress;
    use rusqlite::Connection;

    use crate::{
        auth::auth_guard,
        db::{initialize, Insert},
        models::{NewUser, PasswordHash, ValidatedPassword},
        routes::{endpoints, get_index_page},
        AppState,
    };

    fn get_test_app_config() -> AppState {
        let db_connection =
            Connection::open_in_memory().expect("Could not open database in memory.");
        initialize(&db_connection).expect("Could not initialize database.");

        NewUser {
            email: EmailAddress::new_unchecked("test@test.com"),
            password_hash: PasswordHash::new(ValidatedPassword::new_unchecked("test".to_string()))
                .unwrap(),
        }
        .insert(&db_connection)
        .unwrap();

        AppState::new(db_connection, "42".to_string())
    }

    #[tokio::test]
    async fn root_redirects_to_dashboard() {
        let app_state = get_test_app_config();
        let app = Router::new()
            .route(endpoints::ROOT, get(get_index_page))
            .layer(middleware::from_fn_with_state(
                app_state.clone(),
                auth_guard,
            ))
            .route(endpoints::LOG_IN, get(get_index_page))
            .with_state(app_state);
        let server = TestServer::new(app).expect("Could not create test server.");

        let response = server.get(endpoints::ROOT).await;

        response.assert_status_see_other();
        assert_eq!(response.header("location"), endpoints::LOG_IN);
    }
}

#[cfg(test)]
mod dashboard_route_tests {
    use axum::{
        middleware,
        routing::{get, post},
        Router,
    };
    use axum_extra::extract::cookie::Cookie;
    use axum_test::TestServer;
    use email_address::EmailAddress;
    use rusqlite::Connection;
    use time::{Duration, OffsetDateTime};

    use crate::auth::LogInData;
    use crate::{
        auth::{auth_guard, COOKIE_USER_ID},
        db::{initialize, Insert},
        models::{NewUser, PasswordHash, ValidatedPassword},
        routes::endpoints,
        AppState,
    };

    use super::{get_dashboard_page, log_in};

    fn get_test_server() -> TestServer {
        let db_connection =
            Connection::open_in_memory().expect("Could not open database in memory.");
        initialize(&db_connection).expect("Could not initialize database.");

        NewUser {
            email: EmailAddress::new_unchecked("test@test.com"),
            password_hash: PasswordHash::new(ValidatedPassword::new_unchecked("test".to_string()))
                .unwrap(),
        }
        .insert(&db_connection)
        .unwrap();

        let state = AppState::new(db_connection, "42".to_string());
        let app = Router::new()
            .route(endpoints::DASHBOARD, get(get_dashboard_page))
            .layer(middleware::from_fn_with_state(state.clone(), auth_guard))
            .route(endpoints::LOG_IN, post(log_in))
            .with_state(state);

        TestServer::new(app).expect("Could not create test server.")
    }

    #[tokio::test]
    async fn dashboard_redirects_to_log_in_without_auth_cookie() {
        let server = get_test_server();

        let response = server.get(endpoints::DASHBOARD).await;

        response.assert_status_see_other();
        assert_eq!(response.header("location"), endpoints::LOG_IN);
    }

    #[tokio::test]
    async fn dashboard_redirects_to_log_in_with_invalid_auth_cookie() {
        let server = get_test_server();

        let fake_auth_cookie = Cookie::build((COOKIE_USER_ID, "1"))
            .secure(true)
            .http_only(true)
            .same_site(axum_extra::extract::cookie::SameSite::Lax)
            .build();
        let response = server
            .get(endpoints::DASHBOARD)
            .add_cookie(fake_auth_cookie)
            .await;

        response.assert_status_see_other();
        assert_eq!(response.header("location"), endpoints::LOG_IN);
    }

    #[tokio::test]
    async fn dashboard_redirects_to_log_in_with_expired_auth_cookie() {
        let server = get_test_server();
        let mut expired_auth_cookie = server
            .post(endpoints::LOG_IN)
            .form(&LogInData {
                email: "test@test.com".to_string(),
                password: "test".to_string(),
            })
            .await
            .cookie(COOKIE_USER_ID);

        expired_auth_cookie.set_expires(OffsetDateTime::now_utc() - Duration::weeks(1));

        let response = server
            .get(endpoints::DASHBOARD)
            .add_cookie(expired_auth_cookie)
            .await;

        response.assert_status_see_other();
        assert_eq!(response.header("location"), endpoints::LOG_IN);
    }

    #[tokio::test]
    async fn dashboard_displays_with_auth_cookie() {
        let server = get_test_server();

        let auth_cookie = server
            .post(endpoints::LOG_IN)
            .form(&LogInData {
                email: "test@test.com".to_string(),
                password: "test".to_string(),
            })
            .await
            .cookie(COOKIE_USER_ID);

        server
            .get(endpoints::DASHBOARD)
            .add_cookie(auth_cookie)
            .await
            .assert_status_ok();
    }
}

#[cfg(test)]
mod category_tests {
    use axum_extra::extract::cookie::Cookie;
    use axum_test::TestServer;
    use rusqlite::Connection;
    use serde_json::json;

    use crate::auth::LogInData;
    use crate::{
        auth::COOKIE_USER_ID,
        db::initialize,
        models::{Category, CategoryName, UserID},
        routes::endpoints,
        AppState,
    };

    use super::{build_router, register::RegisterForm};

    fn get_test_app_config() -> AppState {
        let db_connection =
            Connection::open_in_memory().expect("Could not open database in memory.");
        initialize(&db_connection).expect("Could not initialize database.");

        AppState::new(db_connection, "42".to_string())
    }

    async fn create_app_with_user() -> (TestServer, UserID, Cookie<'static>) {
        let state = get_test_app_config();
        let app = build_router(state.clone());

        let server = TestServer::new(app).expect("Could not create test server.");

        let email = "test@test.com";
        let password = "averylongandsecurepassword";

        let response = server
            .post(endpoints::USERS)
            .form(&RegisterForm {
                email: email.to_string(),
                password: password.to_string(),
                confirm_password: password.to_string(),
            })
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
            .post(endpoints::CATEGORIES)
            .add_cookie(auth_cookie.clone())
            .content_type("application/json")
            .json(&json!({
                "id": 0,
                "name": "foo",
                "user_id": user_id,
            }))
            .await
            .json::<Category>();

        (server, user_id, auth_cookie, category)
    }

    #[tokio::test]
    async fn create_category() {
        let (server, user_id, auth_cookie) = create_app_with_user().await;

        let name = CategoryName::new("Foo".to_string()).unwrap();

        let response = server
            .post(endpoints::CATEGORIES)
            .add_cookie(auth_cookie)
            .content_type("application/json")
            .json(&json!({
                "id": 0,
                "name": name,
                "user_id": user_id,
            }))
            .await;

        response.assert_status_ok();

        let category = response.json::<Category>();

        assert_eq!(category.name(), &name);
        assert_eq!(category.user_id(), user_id);
    }

    #[tokio::test]
    async fn get_category() {
        let (server, _, auth_cookie, category) = create_app_with_user_and_category().await;

        let response = server
            .get(&format!("{}/{}", endpoints::CATEGORIES, category.id()))
            .add_cookie(auth_cookie)
            .await;

        response.assert_status_ok();

        let selected_category = response.json::<Category>();

        assert_eq!(selected_category, category);
    }

    #[tokio::test]
    async fn get_category_fails_on_wrong_user() {
        let (server, _, _, category) = create_app_with_user_and_category().await;

        let email = "test2@test.com".to_string();
        let password = "averylongandsecurepassword".to_string();

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
            .form(&LogInData {
                email: email.clone(),
                password: password.clone(),
            })
            .await
            .cookie(COOKIE_USER_ID);

        server
            .get(&format!("{}/{}", endpoints::CATEGORIES, category.id()))
            .add_cookie(auth_cookie)
            .await
            .assert_status_not_found();
    }
}

#[cfg(test)]
mod transaction_tests {
    use axum_extra::extract::cookie::Cookie;
    use axum_test::TestServer;
    use rusqlite::Connection;
    use serde_json::json;
    use time::OffsetDateTime;

    use crate::auth::LogInData;
    use crate::{
        auth::COOKIE_USER_ID,
        db::initialize,
        models::{Category, Transaction, UserID},
        routes::endpoints,
        AppState,
    };

    use super::{build_router, register::RegisterForm};

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
            .post(endpoints::CATEGORIES)
            .add_cookie(auth_cookie.clone())
            .content_type("application/json")
            .json(&json!({
                "id": 0,
                "name": "foo",
                "user_id": user_id,
            }))
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
            .post(endpoints::TRANSACTIONS)
            .add_cookie(auth_cookie)
            .content_type("application/json")
            .json(&json!({
                "id": 0,
                "amount": amount,
                "date": date,
                "description": description,
                "category_id": category.id(),
                "user_id": user_id,
            }))
            .await;

        response.assert_status_ok();

        let transaction = response.json::<Transaction>();

        assert_eq!(transaction.amount(), amount);
        assert_eq!(*transaction.date(), date);
        assert_eq!(transaction.description(), description);
        assert_eq!(transaction.category_id(), category.id());
        assert_eq!(transaction.user_id(), user_id);
    }

    #[tokio::test]
    async fn get_transaction() {
        let (server, user_id, auth_cookie, category) = create_app_with_user_and_category().await;

        let amount = -10.0;
        let date = OffsetDateTime::now_utc();
        let description = "A thingymajig";

        let inserted_transaction = server
            .post(endpoints::TRANSACTIONS)
            .add_cookie(auth_cookie.clone())
            .content_type("application/json")
            .json(&json!({
                "id": 0,
                "amount": amount,
                "date": date,
                "description": description,
                "category_id": category.id(),
                "user_id": user_id,
            }))
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
            .post(endpoints::TRANSACTIONS)
            .add_cookie(auth_cookie.clone())
            .content_type("application/json")
            .json(&json!({
                "id": 0,
                "amount": amount,
                "date": date,
                "description": description,
                "category_id": category.id(),
                "user_id": user_id,
            }))
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
