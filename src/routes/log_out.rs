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
    use axum::{
        body::Body,
        http::{Response, StatusCode},
    };
    use axum_extra::extract::{
        cookie::{Cookie, Expiration, Key},
        PrivateCookieJar,
    };
    use sha2::{Digest, Sha512};
    use time::{Duration, OffsetDateTime};

    use crate::{
        auth::set_auth_cookie,
        models::UserID,
        routes::{endpoints, log_out::get_log_out},
    };

    #[tokio::test]
    async fn log_out_invalidates_auth_cookie_and_redirects() {
        let cookie_jar = set_auth_cookie(get_jar(), UserID::new(123));

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
        let cookie_string = response
            .headers()
            .get("set-cookie")
            .unwrap()
            .to_str()
            .unwrap();
        let auth_cookie = Cookie::parse(cookie_string).unwrap();

        assert_eq!(auth_cookie.max_age(), Some(Duration::ZERO));
        assert_eq!(
            auth_cookie.expires(),
            Some(Expiration::DateTime(OffsetDateTime::UNIX_EPOCH))
        );
    }
}
