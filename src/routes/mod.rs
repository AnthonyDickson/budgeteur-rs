//! This module defines the REST API's routes and their handlers.

use askama::Template;
use axum::{
    extract::{Path, State},
    http::{StatusCode, Uri},
    middleware,
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
    Form, Json, Router,
};
use axum_extra::extract::PrivateCookieJar;
use axum_htmx::HxRedirect;

use dashboard::get_dashboard_page;
use log_in::{get_log_in_page, post_log_in};
use log_out::get_log_out;
use register::{create_user, get_register_page};
use serde::{Deserialize, Serialize};
use tower_http::services::ServeDir;
use transaction::{create_transaction, get_transaction};

use crate::{
    auth::{auth_guard, get_user_id_from_auth_cookie},
    models::{Category, CategoryName, DatabaseID, UserID},
    AppError, AppState, HtmlTemplate,
};

mod dashboard;
pub mod endpoints;
pub mod log_in;
mod log_out;
mod navigation;
pub mod register;
mod templates;
mod transaction;

/// Return a router with all the app's routes.
pub fn build_router(state: AppState) -> Router {
    let unprotected_routes = Router::new()
        .route(endpoints::COFFEE, get(get_coffee))
        .route(endpoints::LOG_IN, get(get_log_in_page))
        .route(endpoints::LOG_IN, post(post_log_in))
        .route(endpoints::LOG_OUT, get(get_log_out))
        .route(endpoints::REGISTER, get(get_register_page))
        .route(endpoints::USERS, post(create_user))
        .route(
            endpoints::INTERNAL_ERROR,
            get(get_internal_server_error_page),
        );

    let protected_routes = Router::new()
        .route(endpoints::ROOT, get(get_index_page))
        .route(endpoints::DASHBOARD, get(get_dashboard_page))
        .route(endpoints::USER_CATEGORIES, post(create_category))
        .route(endpoints::CATEGORY, get(get_category))
        .route(endpoints::USER_TRANSACTIONS, post(create_transaction))
        .route(endpoints::TRANSACTION, get(get_transaction))
        .layer(middleware::from_fn_with_state(state.clone(), auth_guard));

    protected_routes
        .merge(unprotected_routes)
        .nest_service("/assets", ServeDir::new("assets/"))
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

#[derive(Debug, Serialize, Deserialize)]
struct CategoryData {
    name: String,
}

/// A route handler for creating a new category.
///
/// # Panics
///
/// Panics if the lock for the database connection is already held by the same thread.
async fn create_category(
    State(state): State<AppState>,
    Path(user_id): Path<UserID>,
    _: PrivateCookieJar,
    Form(new_category): Form<CategoryData>,
) -> impl IntoResponse {
    let connection_mutex = state.db_connection();
    let connection = connection_mutex.lock().unwrap();

    let name = CategoryName::new(new_category.name)?;

    Category::build(name, user_id)
        .insert(&connection)
        .map(|category| (StatusCode::OK, Json(category)))
        .map_err(AppError::CategoryError)
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
        .map_err(AppError::CategoryError)
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

#[cfg(test)]
mod root_route_tests {
    use axum::{middleware, routing::get, Router};
    use axum_test::TestServer;
    use email_address::EmailAddress;
    use rusqlite::Connection;

    use crate::{
        auth::auth_guard,
        db::initialize,
        models::{PasswordHash, User, ValidatedPassword},
        routes::{endpoints, get_index_page},
        AppState,
    };

    fn get_test_app_config() -> AppState {
        let db_connection =
            Connection::open_in_memory().expect("Could not open database in memory.");
        initialize(&db_connection).expect("Could not initialize database.");

        User::build(
            EmailAddress::new_unchecked("test@test.com"),
            PasswordHash::new(ValidatedPassword::new_unchecked("test".to_string())).unwrap(),
        )
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
mod category_tests {
    use axum_extra::extract::cookie::Cookie;
    use axum_test::TestServer;
    use rusqlite::Connection;

    use crate::auth::LogInData;
    use crate::{
        auth::COOKIE_USER_ID,
        db::initialize,
        models::{Category, CategoryName, UserID},
        routes::endpoints,
        AppState,
    };

    use super::CategoryData;
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
            .post(&endpoints::USER_CATEGORIES.replace(":user_id", &user_id.to_string()))
            .add_cookie(auth_cookie.clone())
            .content_type("application/json")
            .form(&CategoryData {
                name: "foo".to_string(),
            })
            .await
            .json::<Category>();

        (server, user_id, auth_cookie, category)
    }

    #[tokio::test]
    async fn create_category() {
        let (server, user_id, auth_cookie) = create_app_with_user().await;

        let name = CategoryName::new("Foo".to_string()).unwrap();

        let response = server
            .post(&endpoints::USER_CATEGORIES.replace(":user_id", &user_id.to_string()))
            .add_cookie(auth_cookie)
            .content_type("application/json")
            .form(&CategoryData {
                name: String::from("Foo"),
            })
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
