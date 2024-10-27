//! This file defines the high-level log-out route logic.
//! The underlying auth logic is handled by the auth module.

use axum::response::{IntoResponse, Redirect, Response};
use axum_extra::extract::PrivateCookieJar;

use crate::auth::invalidate_auth_cookie;

use super::endpoints;

/// Invalidate the auth cookie and redirect the client to the log-in page.
pub async fn get_log_out(jar: PrivateCookieJar) -> Response {
    let jar = invalidate_auth_cookie(jar);

    (jar, Redirect::to(endpoints::LOG_IN)).into_response()
}

#[cfg(test)]
mod log_out_tests {
    use axum::{routing::post, Router};
    use axum_extra::extract::cookie::Expiration;
    use axum_test::TestServer;
    use rusqlite::Connection;
    use time::{Duration, OffsetDateTime};

    use crate::{
        auth::{LogInData, COOKIE_USER_ID},
        db::initialize,
        models::{PasswordHash, ValidatedPassword},
        routes::{endpoints, log_in::post_log_in, log_out::get_log_out},
        stores::UserStore,
        AppState,
    };

    fn get_test_app_config() -> AppState {
        let db_connection =
            Connection::open_in_memory().expect("Could not open database in memory.");
        initialize(&db_connection).expect("Could not initialize database.");

        let state = AppState::new(db_connection, "42");

        state
            .user_store()
            .create(
                "test@test.com".parse().unwrap(),
                PasswordHash::new(ValidatedPassword::new_unchecked("test".to_string())).unwrap(),
            )
            .unwrap();

        state
    }

    #[tokio::test]
    async fn log_out_invalidates_auth_cookie_and_redirects() {
        let app = Router::new()
            .route(endpoints::LOG_IN, post(post_log_in))
            .route(endpoints::LOG_OUT, post(get_log_out))
            .with_state(get_test_app_config());

        let server = TestServer::new(app).expect("Could not create test server.");

        server
            .post(endpoints::LOG_IN)
            .form(&LogInData {
                email: "test@test.com".to_string(),
                password: "test".to_string(),
            })
            .await
            .assert_status_see_other();

        let response = server.post(endpoints::LOG_OUT).await;
        response.assert_status_see_other();

        let auth_cookie = response.cookie(COOKIE_USER_ID);

        assert_eq!(auth_cookie.max_age(), Some(Duration::ZERO));
        assert_eq!(
            auth_cookie.expires(),
            Some(Expiration::DateTime(OffsetDateTime::UNIX_EPOCH))
        );
    }
}
