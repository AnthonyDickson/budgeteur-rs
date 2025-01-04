//! This file defines the high-level log-out route logic.
//! The underlying auth logic is handled by the auth module.

use axum::response::{IntoResponse, Redirect, Response};
use axum_extra::extract::PrivateCookieJar;

use crate::auth::cookie::invalidate_auth_cookie;

use super::endpoints;

/// Invalidate the auth cookie and redirect the client to the log-in page.
pub async fn get_log_out(jar: PrivateCookieJar) -> Response {
    let jar = invalidate_auth_cookie(jar);

    (jar, Redirect::to(endpoints::LOG_IN)).into_response()
}

#[cfg(test)]
mod log_out_tests {
    use axum::{
        body::Body,
        http::{header::SET_COOKIE, Response, StatusCode},
    };
    use axum_extra::extract::{
        cookie::{Cookie, Key},
        PrivateCookieJar,
    };
    use sha2::{Digest, Sha512};
    use time::{Duration, OffsetDateTime};

    use crate::{
        auth::cookie::{set_auth_cookie, COOKIE_USER_ID, DEFAULT_COOKIE_DURATION},
        models::UserID,
        routes::{endpoints, log_out::get_log_out},
    };

    #[tokio::test]
    async fn log_out_invalidates_auth_cookie_and_redirects() {
        let cookie_jar =
            set_auth_cookie(get_jar(), UserID::new(123), DEFAULT_COOKIE_DURATION).unwrap();

        let response = get_log_out(cookie_jar).await;

        assert_redirect(&response, endpoints::LOG_IN);
        assert_cookie_expired(&response);
    }

    fn get_jar() -> PrivateCookieJar {
        let key = Key::from(&Sha512::digest("42"));
        PrivateCookieJar::new(key)
    }

    fn assert_redirect(response: &Response<Body>, want_location: &str) {
        let redirect_location = response.headers().get("location").unwrap();

        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        assert_eq!(redirect_location, want_location);
    }

    fn assert_cookie_expired(response: &Response<Body>) {
        for cookie_header in response.headers().get_all(SET_COOKIE) {
            let cookie_string = cookie_header.to_str().unwrap();
            let cookie = Cookie::parse(cookie_string).unwrap();

            if cookie.name() != COOKIE_USER_ID || cookie.name() != COOKIE_USER_ID {
                continue;
            }

            assert_eq!(
                cookie.expires_datetime(),
                Some(OffsetDateTime::UNIX_EPOCH),
                "got expires {:?}, want {:?}",
                cookie.expires_datetime(),
                Some(OffsetDateTime::UNIX_EPOCH),
            );

            assert_eq!(
                cookie.max_age(),
                Some(Duration::ZERO),
                "got max age {:?}, want {:?}",
                cookie.max_age(),
                Some(Duration::ZERO),
            );
        }
    }
}
