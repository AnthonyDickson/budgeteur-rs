//! This module defines the REST API's routes and their handlers.

use askama_axum::Template;
use axum::{
    Router,
    http::{StatusCode, Uri},
    middleware,
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
};
use axum_htmx::HxRedirect;

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
mod views;

use category::{create_category, get_category};
use dashboard::get_dashboard_page;
use log_in::{get_log_in_page, post_log_in};
use log_out::get_log_out;
use register::{create_user, get_register_page};
use tower_http::services::ServeDir;
use transaction::{create_transaction, get_transaction};
use transactions::get_transactions_page;
use views::new_transaction::get_new_transaction_page;

use crate::{
    auth::middleware::{auth_guard, auth_guard_hx},
    stores::sql_store::SQLAppState,
};

/// Return a router with all the app's routes.
pub fn build_router(state: SQLAppState) -> Router {
    let unprotected_routes = Router::new()
        .route(endpoints::COFFEE, get(get_coffee))
        .route(endpoints::LOG_IN_VIEW, get(get_log_in_page))
        .route(endpoints::LOG_IN_API, post(post_log_in))
        .route(endpoints::LOG_OUT, get(get_log_out))
        .route(endpoints::REGISTER_VIEW, get(get_register_page))
        .route(endpoints::USERS, post(create_user))
        .route(
            endpoints::INTERNAL_ERROR_VIEW,
            get(get_internal_server_error_page),
        );

    let protected_routes = Router::new()
        .route(endpoints::ROOT, get(get_index_page))
        .route(endpoints::DASHBOARD_VIEW, get(get_dashboard_page))
        .route(endpoints::CATEGORY, get(get_category))
        .route(endpoints::TRANSACTION, get(get_transaction))
        .route(endpoints::TRANSACTIONS_API, get(get_transactions_page))
        .route(endpoints::TRANSACTIONS_VIEW, get(get_transactions_page))
        .route(
            endpoints::NEW_TRANSACTION_VIEW,
            get(get_new_transaction_page),
        )
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
    Redirect::to(endpoints::DASHBOARD_VIEW)
}

/// Get a response that will redirect the client to the internal server error 500 page.
///
/// **Note**: This redirect is intended to be served as a response to a POST request initiated by HTMX.
/// Route handlers using GET should use `axum::response::Redirect` to redirect via a response.
pub(crate) fn get_internal_server_error_redirect() -> Response {
    (
        HxRedirect(Uri::from_static(endpoints::INTERNAL_ERROR_VIEW)),
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
    use askama_axum::IntoResponse;
    use axum::http::StatusCode;

    use crate::routes::{endpoints, get_index_page};

    #[tokio::test]
    async fn root_redirects_to_dashboard() {
        let response = get_index_page().await.into_response();
        assert_eq!(response.status(), StatusCode::SEE_OTHER);

        let location = response.headers().get("location").unwrap();
        assert_eq!(location, endpoints::DASHBOARD_VIEW);
    }
}
