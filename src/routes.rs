use std::str::FromStr;

use askama::Template;
use axum::{
    debug_handler,
    extract::{Path, State},
    http::{StatusCode, Uri},
    middleware,
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
    Extension, Form, Json, Router,
};
use axum_extra::extract::PrivateCookieJar;
use axum_htmx::HxRedirect;
use email_address::EmailAddress;
use serde::{Deserialize, Serialize};

use crate::{
    auth::{auth_guard, get_user_id_from_auth_cookie, set_auth_cookie, sign_in},
    db::{DbError, Insert, SelectBy},
    model::{
        Category, DatabaseID, NewCategory, NewTransaction, NewUser, PasswordHash, RawPassword,
        Transaction, UserID,
    },
    AppError, AppState, HtmlTemplate,
};

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
}

// TODO: Update existing routes to respond with HTML
/// Return a router with all the app's routes.
pub fn build_router(state: AppState) -> Router {
    let unprotected_routes = Router::new()
        .route(endpoints::COFFEE, get(get_coffee))
        .route(endpoints::LOG_IN, get(get_sign_in_page))
        .route(endpoints::LOG_IN, post(sign_in))
        .route(endpoints::REGISTER, get(get_register_page))
        .route(endpoints::USERS, post(create_user));

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
#[template(path = "views/log_in.html")]
struct SignInTemplate<'a> {
    register_route: &'a str,
}

/// Display the sign-in page.
async fn get_sign_in_page() -> Response {
    HtmlTemplate(SignInTemplate {
        register_route: endpoints::REGISTER,
    })
    .into_response()
}

// TODO: Create module for routes and move register page code to own file.

#[derive(Template)]
#[template(path = "views/register.html")]
struct RegisterPageTemplate<'a> {
    register_form: RegisterFormTemplate<'a>,
}

#[derive(Template)]
#[template(path = "partials/register/form.html")]
struct RegisterFormTemplate<'a> {
    log_in_route: &'a str,
    create_user_route: &'a str,
    email_input: EmailInputTemplate<'a>,
    password_input: PasswordInputTemplate<'a>,
    confirm_password_input: ConfirmPasswordInputTemplate<'a>,
}

impl Default for RegisterFormTemplate<'_> {
    fn default() -> Self {
        Self {
            log_in_route: endpoints::LOG_IN,
            create_user_route: endpoints::USERS,
            email_input: EmailInputTemplate::default(),
            password_input: PasswordInputTemplate::default(),
            confirm_password_input: ConfirmPasswordInputTemplate::default(),
        }
    }
}

#[derive(Template)]
#[template(path = "partials/register/inputs/email.html")]
struct EmailInputTemplate<'a> {
    value: &'a str,
    error_message: &'a str,
    validation_route: &'a str,
}

impl Default for EmailInputTemplate<'_> {
    fn default() -> Self {
        Self {
            value: "",
            error_message: "",
            validation_route: endpoints::USERS,
        }
    }
}

#[derive(Template, Default)]
#[template(path = "partials/register/inputs/password.html")]
struct PasswordInputTemplate<'a> {
    error_message: &'a str,
}

#[derive(Template, Default)]
#[template(path = "partials/register/inputs/confirm_password.html")]
struct ConfirmPasswordInputTemplate<'a> {
    error_message: &'a str,
}

/// Display the registration page.
async fn get_register_page() -> Response {
    HtmlTemplate(RegisterPageTemplate {
        register_form: RegisterFormTemplate::default(),
    })
    .into_response()
}

#[derive(Serialize, Deserialize)]
struct RegisterForm {
    email: String,
    password: String,
    confirm_password: String,
}

#[debug_handler]
async fn create_user(
    State(state): State<AppState>,
    jar: PrivateCookieJar,
    Form(user_data): Form<RegisterForm>,
) -> Response {
    if user_data.password != user_data.confirm_password {
        return HtmlTemplate(RegisterFormTemplate {
            email_input: EmailInputTemplate {
                value: &user_data.email,
                ..EmailInputTemplate::default()
            },
            confirm_password_input: ConfirmPasswordInputTemplate {
                error_message: "Passwords do not match",
            },
            ..RegisterFormTemplate::default()
        })
        .into_response();
    }

    let email = match EmailAddress::from_str(&user_data.email) {
        Ok(email) => email,
        // Due to the client-side validation, the below error will not happen very often, but it still pays to check.
        Err(e) => {
            return HtmlTemplate(RegisterFormTemplate {
                email_input: EmailInputTemplate {
                    value: &user_data.email,
                    error_message: &format!("Invalid email address: {}", e),
                    ..EmailInputTemplate::default()
                },
                ..RegisterFormTemplate::default()
            })
            .into_response();
        }
    };

    let raw_password = match RawPassword::new(user_data.password) {
        Ok(password) => password,
        Err(e) => {
            return HtmlTemplate(RegisterFormTemplate {
                email_input: EmailInputTemplate {
                    value: &user_data.email,
                    ..EmailInputTemplate::default()
                },
                password_input: PasswordInputTemplate {
                    error_message: e.to_string().as_ref(),
                },
                ..RegisterFormTemplate::default()
            })
            .into_response();
        }
    };

    let password_hash = match PasswordHash::new(raw_password) {
        Ok(hash) => hash,
        Err(e) => {
            tracing::error!("an error occurred while hashing a password: {e}");

            return HtmlTemplate(RegisterFormTemplate {
                email_input: EmailInputTemplate {
                    value: &user_data.email,
                    ..EmailInputTemplate::default()
                },
                password_input: PasswordInputTemplate {
                    error_message: "An internal server error ocurred. You can either try again later, or try again with a different password",
                },
                ..RegisterFormTemplate::default()
            })
            .into_response();
        }
    };

    // TODO: Abstract away database interactions into a 'repository' struct. The repo should handle the CRUD operations.
    // Routes should ideally should be simple and high-level. I should aim to have one function call to a repo, and then another function call to render a template.
    NewUser {
        email,
        password_hash,
    }
    .insert(&state.db_connection().lock().unwrap())
    .map(|user| {
        let jar = set_auth_cookie(jar, user.id());

        (
            StatusCode::SEE_OTHER,
            HxRedirect(Uri::from_static(endpoints::LOG_IN)),
            jar,
        )
    })
    // TODO: Render error in form.
    .map_err(|e| match e {
        DbError::DuplicateEmail => HtmlTemplate(RegisterFormTemplate {
            email_input: EmailInputTemplate {
                value: &user_data.email,
                error_message: &format!("The email address {} is already in use", &user_data.email),
                ..EmailInputTemplate::default()
            },
            ..RegisterFormTemplate::default()
        })
        .into_response(),
        DbError::DuplicatePassword => HtmlTemplate(RegisterFormTemplate {
            email_input: EmailInputTemplate {
                value: &user_data.email,
                ..EmailInputTemplate::default()
            },
            password_input: PasswordInputTemplate {
                error_message: "The password is already in use",
            },
            ..RegisterFormTemplate::default()
        })
        .into_response(),
        // TODO: Render form with error message indicating a internal server error.
        _ => AppError::UserCreation(format!("Could not create user: {e:?}")).into_response(),
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
mod root_route_tests {
    use axum::{middleware, routing::get, Router};
    use axum_test::TestServer;
    use email_address::EmailAddress;
    use rusqlite::Connection;

    use crate::{
        auth::auth_guard,
        db::{initialize, Insert},
        model::{NewUser, PasswordHash, RawPassword},
        routes::{endpoints, get_index_page},
        AppState,
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
    use serde_json::json;
    use time::{Duration, OffsetDateTime};

    use crate::{
        auth::{auth_guard, sign_in, COOKIE_USER_ID},
        db::{initialize, Insert},
        model::{NewUser, PasswordHash, RawPassword},
        routes::endpoints,
        AppState,
    };

    use super::get_dashboard_page;

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
            .route(endpoints::DASHBOARD, get(get_dashboard_page))
            .layer(middleware::from_fn_with_state(state.clone(), auth_guard))
            .route(endpoints::LOG_IN, post(sign_in))
            .with_state(state);

        TestServer::new(app).expect("Could not create test server.")
    }

    #[tokio::test]
    async fn dashboard_redirects_to_sign_in_without_auth_cookie() {
        let server = get_test_server();

        let response = server.get(endpoints::DASHBOARD).await;

        response.assert_status_see_other();
        assert_eq!(response.header("location"), endpoints::LOG_IN);
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
            .get(endpoints::DASHBOARD)
            .add_cookie(fake_auth_cookie)
            .await;

        response.assert_status_see_other();
        assert_eq!(response.header("location"), endpoints::LOG_IN);
    }

    #[tokio::test]
    async fn dashboard_redirects_to_sign_in_with_expired_auth_cookie() {
        let server = get_test_server();
        let mut expired_auth_cookie = server
            .post(endpoints::LOG_IN)
            .json(&json!({
                "email": "test@test.com",
                "password": "test"
            }))
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
            .json(&json!({
                "email": "test@test.com",
                "password": "test"
            }))
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
mod user_tests {

    use axum::{routing::post, Router};
    use axum_test::TestServer;
    use rusqlite::Connection;
    use serde::{Deserialize, Serialize};

    use crate::{
        db::initialize,
        routes::{create_user, endpoints, RegisterForm},
        AppState,
    };

    fn get_test_app_config() -> AppState {
        let db_connection =
            Connection::open_in_memory().expect("Could not open database in memory.");
        initialize(&db_connection).expect("Could not initialize database.");

        AppState::new(db_connection, "42".to_string())
    }

    #[derive(Serialize, Deserialize)]
    struct Foo {
        bar: String,
    }

    #[tokio::test]
    async fn create_user_succeeds() {
        let app = Router::new()
            .route(endpoints::USERS, post(create_user))
            .with_state(get_test_app_config());

        let server = TestServer::new(app).expect("Could not create test server.");

        server
            .post(endpoints::USERS)
            .form(&RegisterForm {
                email: "foo@bar.baz".to_string(),
                password: "iamtestingwhethericancreateanewuser".to_string(),
                confirm_password: "iamtestingwhethericancreateanewuser".to_string(),
            })
            .await
            .assert_status_see_other();
    }

    #[tokio::test]
    async fn create_user_fails_when_passwords_do_not_match() {
        let app = Router::new()
            .route(endpoints::USERS, post(create_user))
            .with_state(get_test_app_config());

        let server = TestServer::new(app).expect("Could not create test server.");

        let response = server
            .post(endpoints::USERS)
            .form(&RegisterForm {
                email: "foo@".to_string(),
                password: "iamtestingwhethericancreateanewuser".to_string(),
                confirm_password: "thisisadifferentpassword".to_string(),
            })
            .await
            .text();

        assert!(response.to_lowercase().contains("passwords do not match"))
    }
}

#[cfg(test)]
mod category_tests {
    use axum_extra::extract::cookie::Cookie;
    use axum_test::TestServer;
    use email_address::EmailAddress;
    use rusqlite::Connection;
    use serde_json::json;

    use crate::{
        auth::COOKIE_USER_ID,
        db::initialize,
        model::{Category, CategoryName, UserID},
        routes::endpoints,
        AppState,
    };

    use super::{build_router, RegisterForm};

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

        let email = "test2@test.com";
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

        let auth_cookie = server
            .post(endpoints::LOG_IN)
            .content_type("application/json")
            .json(&json!({
                "email": EmailAddress::new_unchecked(email),
                "password": password
            }))
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
    use chrono::Utc;
    use rusqlite::Connection;
    use serde_json::json;

    use crate::{
        auth::COOKIE_USER_ID,
        db::initialize,
        model::{Category, Transaction, UserID},
        routes::{endpoints, RegisterForm},
        AppState,
    };

    use super::build_router;

    fn get_test_app_config() -> AppState {
        let db_connection =
            Connection::open_in_memory().expect("Could not open database in memory.");
        initialize(&db_connection).expect("Could not initialize database.");

        AppState::new(db_connection, "42".to_string())
    }

    async fn create_app_with_user() -> (TestServer, UserID, Cookie<'static>) {
        let app = build_router(get_test_app_config());

        let server = TestServer::new(app).expect("Could not create test server.");

        let email = "test@test.com";
        let password = "averysafeandsecurepassword";

        let response = server
            .post(endpoints::USERS)
            .form(&RegisterForm {
                email: email.to_string(),
                password: password.to_string(),
                confirm_password: password.to_string(),
            })
            .await;

        response.assert_status_see_other();

        let response = server
            .post(endpoints::LOG_IN)
            .content_type("application/json")
            .json(&json!({
                "email": email,
                "password": password,
            }))
            .await;

        response.assert_status_ok();
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
        let date = Utc::now().date_naive();
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
        let date = Utc::now().date_naive();
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
        let date = Utc::now().date_naive();
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

        let email = "test2@test.com";
        let password = "averystrongandsecurepassword";

        let response = server
            .post(endpoints::USERS)
            .form(&RegisterForm {
                email: email.to_string(),
                password: password.to_string(),
                confirm_password: password.to_string(),
            })
            .await;

        response.assert_status_see_other();

        let auth_cookie = server
            .post(endpoints::LOG_IN)
            .content_type("application/json")
            .json(&json!({
                "email": email,
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
