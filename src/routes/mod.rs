//! This module defines the REST API's routes and their handlers.

use askama_axum::Template;
use axum::{
    http::{StatusCode, Uri},
    middleware,
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
    Router,
};
use axum_htmx::HxRedirect;

use category::{create_category, get_category};
use dashboard::get_dashboard_page;
use log_in::{get_log_in_page, post_log_in};
use log_out::get_log_out;
use register::{create_user, get_register_page};
use tower_http::services::ServeDir;
use transaction::{create_transaction, get_transaction};
use transactions::get_transactions_page;

use crate::{
    auth::{auth_guard, auth_guard_hx},
    AppState,
};

mod category;
mod dashboard;
pub mod endpoints;
mod log_in;
mod log_out;
mod navigation;
mod register;
mod templates;
mod transaction;
mod transactions;

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
        .route(endpoints::CATEGORY, get(get_category))
        .route(endpoints::TRANSACTION, get(get_transaction))
        .route(endpoints::TRANSACTIONS, get(get_transactions_page))
        .layer(middleware::from_fn_with_state(state.clone(), auth_guard));

    // These POST routes need to use the HX-REDIRECT header for auth redirects to work properly for
    // HTMX requests.
    let protected_routes = protected_routes.merge(
        Router::new()
            .route(endpoints::USER_CATEGORIES, post(create_category))
            .route(endpoints::USER_TRANSACTIONS, post(create_transaction))
            .layer(middleware::from_fn_with_state(state.clone(), auth_guard_hx)),
    );

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
    InternalServerErrorPageTemplate.into_response()
}

#[derive(Template)]
#[template(path = "views/not_found_404.html")]
struct NotFoundTemplate;

async fn get_404_not_found() -> Response {
    (StatusCode::NOT_FOUND, NotFoundTemplate).into_response()
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

        AppState::new(db_connection, "42")
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
