//! Application router configuration with protected and unprotected route definitions.

use askama::Template;
use axum::{
    Router,
    http::StatusCode,
    middleware,
    response::{Html, IntoResponse, Redirect, Response},
    routing::{delete, get, post, put},
};
use axum_htmx::HxRedirect;
use tower_http::services::ServeDir;

use crate::{
    AppState,
    auth_middleware::{auth_guard, auth_guard_hx},
    balance::get_balances_page,
    csv_import::{get_import_page, import_transactions},
    dashboard::{get_dashboard_page, update_excluded_tags},
    endpoints,
    forgot_password::get_forgot_password_page,
    log_in::{get_log_in_page, post_log_in},
    log_out::get_log_out,
    not_found::get_404_not_found,
    register_user::{get_register_page, register_user},
    rule::{
        auto_tag_all_transactions_endpoint, auto_tag_untagged_transactions_endpoint,
        create_rule_endpoint, delete_rule_endpoint, get_edit_rule_page, get_new_rule_page,
        get_rules_page, update_rule_endpoint,
    },
    shared_templates::render,
    tag::{
        create_tag_endpoint, delete_tag_endpoint, get_edit_tag_page, get_new_tag_page,
        update_tag_endpoint,
    },
    tags_page::get_tags_page,
    transaction::{
        create_transaction_endpoint, delete_transaction_endpoint, edit_tranction_endpoint,
        get_create_transaction_page, get_edit_transaction_page, get_transactions_page,
    },
};

/// Return a router with all the app's routes.
pub fn build_router(state: AppState) -> Router {
    let unprotected_routes = Router::new()
        .route(endpoints::COFFEE, get(get_coffee))
        .route(endpoints::LOG_IN_VIEW, get(get_log_in_page))
        .route(endpoints::LOG_IN_API, post(post_log_in))
        .route(endpoints::LOG_OUT, get(get_log_out))
        .route(endpoints::REGISTER_VIEW, get(get_register_page))
        .route(
            endpoints::FORGOT_PASSWORD_VIEW,
            get(get_forgot_password_page),
        )
        .route(endpoints::USERS, post(register_user))
        .route(
            endpoints::INTERNAL_ERROR_VIEW,
            get(get_internal_server_error_page),
        );

    let protected_routes = Router::new()
        .route(endpoints::ROOT, get(get_index_page))
        .route(endpoints::DASHBOARD_VIEW, get(get_dashboard_page))
        .route(endpoints::TRANSACTIONS_VIEW, get(get_transactions_page))
        .route(
            endpoints::NEW_TRANSACTION_VIEW,
            get(get_create_transaction_page),
        )
        .route(
            endpoints::EDIT_TRANSACTION_VIEW,
            get(get_edit_transaction_page),
        )
        .route(endpoints::NEW_TAG_VIEW, get(get_new_tag_page))
        .route(endpoints::EDIT_TAG_VIEW, get(get_edit_tag_page))
        .route(endpoints::TAGS_VIEW, get(get_tags_page))
        .route(endpoints::NEW_RULE_VIEW, get(get_new_rule_page))
        .route(endpoints::EDIT_RULE_VIEW, get(get_edit_rule_page))
        .route(endpoints::RULES_VIEW, get(get_rules_page))
        .route(endpoints::IMPORT_VIEW, get(get_import_page))
        .route(endpoints::BALANCES_VIEW, get(get_balances_page))
        .layer(middleware::from_fn_with_state(state.clone(), auth_guard));

    // These POST/PUT routes need to use the HX-REDIRECT header for auth redirects to work properly for HTMX requests.
    let protected_routes = protected_routes.merge(
        Router::new()
            .route(
                endpoints::TRANSACTIONS_API,
                post(create_transaction_endpoint),
            )
            .route(
                endpoints::DELETE_TRANSACTION,
                delete(delete_transaction_endpoint),
            )
            .route(
                endpoints::EDIT_TRANSACTION_VIEW,
                put(edit_tranction_endpoint),
            )
            .route(endpoints::POST_TAG, post(create_tag_endpoint))
            .route(endpoints::PUT_TAG, put(update_tag_endpoint))
            .route(endpoints::DELETE_TAG, delete(delete_tag_endpoint))
            .route(endpoints::POST_RULE, post(create_rule_endpoint))
            .route(endpoints::PUT_RULE, put(update_rule_endpoint))
            .route(endpoints::DELETE_RULE, delete(delete_rule_endpoint))
            .route(
                endpoints::AUTO_TAG_ALL,
                post(auto_tag_all_transactions_endpoint),
            )
            .route(
                endpoints::AUTO_TAG_UNTAGGED,
                post(auto_tag_untagged_transactions_endpoint),
            )
            .route(endpoints::IMPORT, post(import_transactions))
            .route(
                endpoints::DASHBOARD_EXCLUDED_TAGS,
                post(update_excluded_tags),
            )
            .layer(middleware::from_fn_with_state(state.clone(), auth_guard_hx)),
    );

    protected_routes
        .merge(unprotected_routes)
        .nest_service(endpoints::STATIC, ServeDir::new("static/"))
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
        HxRedirect(endpoints::INTERNAL_ERROR_VIEW.to_owned()),
        StatusCode::INTERNAL_SERVER_ERROR,
    )
        .into_response()
}

#[derive(Template)]
#[template(path = "views/internal_server_error_500.html")]
pub struct InternalServerErrorPageTemplate<'a> {
    pub description: &'a str,
    pub fix: &'a str,
}

impl Default for InternalServerErrorPageTemplate<'_> {
    fn default() -> Self {
        Self {
            description: "Sorry, something went wrong.",
            fix: "Try again later or check the server logs",
        }
    }
}

async fn get_internal_server_error_page() -> Response {
    render_internal_server_error(Default::default())
}

pub fn render_internal_server_error(template: InternalServerErrorPageTemplate) -> Response {
    render(StatusCode::INTERNAL_SERVER_ERROR, template)
}

#[cfg(test)]
mod root_route_tests {
    use axum::{http::StatusCode, response::IntoResponse};

    use crate::{endpoints, routing::get_index_page};

    #[tokio::test]
    async fn root_redirects_to_dashboard() {
        let response = get_index_page().await.into_response();
        assert_eq!(response.status(), StatusCode::SEE_OTHER);

        let location = response.headers().get("location").unwrap();
        assert_eq!(location, endpoints::DASHBOARD_VIEW);
    }
}
