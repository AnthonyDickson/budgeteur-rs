//! This file defines the dashboard route and its handlers.

use super::{
    endpoints::{self},
    navigation::{get_nav_bar, NavbarTemplate},
};
use askama_axum::Template;
use axum::{
    extract::State,
    response::{IntoResponse, Response},
    Extension,
};
use time::OffsetDateTime;

use crate::{
    models::UserID,
    stores::{CategoryStore, TransactionStore, UserStore},
    AppError, AppState,
};

/// Renders the dashboard page.
#[derive(Template)]
#[template(path = "views/dashboard.html")]
struct DashboardTemplate<'a> {
    navbar: NavbarTemplate<'a>,
    user_id: UserID,
    /// How much over or under budget the user is for this week.
    balance: f64,
}

/// Display a page with an overview of the user's data.
pub async fn get_dashboard_page<C, T, U>(
    State(state): State<AppState<C, T, U>>,
    Extension(user_id): Extension<UserID>,
) -> Response
where
    C: CategoryStore + Send + Sync,
    T: TransactionStore + Send + Sync,
    U: UserStore + Send + Sync,
{
    let navbar = get_nav_bar(endpoints::DASHBOARD);

    // TODO: Create function for getting transactions within a time span (time::Duration) in TransactionStore.
    let transactions = state.transaction_store().get_by_user_id(user_id);
    let transactions = match transactions {
        Ok(transactions) => transactions,
        Err(error) => return AppError::TransactionError(error).into_response(),
    };

    let today = OffsetDateTime::now_utc().date();
    let week = today.monday_based_week();
    let balance = transactions
        .iter()
        .filter_map(|transaction| {
            if transaction.date().monday_based_week() == week {
                Some(transaction.amount())
            } else {
                None
            }
        })
        .sum();

    DashboardTemplate {
        navbar,
        user_id,
        balance,
    }
    .into_response()
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
    use rusqlite::Connection;
    use time::{Duration, OffsetDateTime};

    use crate::{
        auth::LogInData,
        models::ValidatedPassword,
        routes::log_in::post_log_in,
        stores::{sql_store::create_app_state, UserStore},
    };
    use crate::{
        auth::{auth_guard, COOKIE_USER_ID},
        models::PasswordHash,
        routes::endpoints,
    };

    use super::get_dashboard_page;

    fn get_test_server() -> TestServer {
        let db_connection =
            Connection::open_in_memory().expect("Could not open database in memory.");

        let mut state = create_app_state(db_connection, "42").unwrap();

        state
            .user_store()
            .create(
                "test@test.com".parse().unwrap(),
                PasswordHash::new(ValidatedPassword::new_unchecked("test".to_string()), 4).unwrap(),
            )
            .unwrap();

        let app = Router::new()
            .route(endpoints::DASHBOARD, get(get_dashboard_page))
            .layer(middleware::from_fn_with_state(state.clone(), auth_guard))
            .route(endpoints::LOG_IN, post(post_log_in))
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

        expired_auth_cookie.set_max_age(Duration::ZERO);
        expired_auth_cookie.set_expires(OffsetDateTime::UNIX_EPOCH);

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
