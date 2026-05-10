//! Defines functions for handling user authentication with cookies.

use axum_extra::extract::{
    PrivateCookieJar,
    cookie::{Cookie, SameSite},
};
use time::{Duration, OffsetDateTime};

use crate::{
    Error,
    auth::{SessionId, Token},
};

/// The name of the cookie that will hold the auth token.
pub const COOKIE_TOKEN: &str = "auth_token";

/// Add a session token to the cookie jar, indicating that a user is logged in
/// and authenticated.
///
/// Returns the cookie jar with the cookie added.
///
/// # Errors
///
/// Returns an [Error::JSONSerializationError] if the session ID cannot be
/// serialized.
pub fn set_auth_cookie(
    jar: PrivateCookieJar,
    session_id: SessionId,
    expiry: OffsetDateTime,
) -> Result<PrivateCookieJar, Error> {
    let token = {
        let token = Token { session_id };

        serde_json::to_string(&token)
            .map_err(|err| Error::JSONSerializationError(err.to_string()))?
    };

    Ok(jar.add(
        Cookie::build((COOKIE_TOKEN, token))
            .expires(expiry)
            .http_only(true)
            .same_site(SameSite::Strict)
            .secure(true)
            // Explicitly set path to root to avoid issues with subpaths.
            // For example, if the cookie is invalidated from a subpath such
            // as 'http://example.com/api/logout', the browser will not
            // invalidate the cookie for 'http://example.com/path1'.
            .path("/"),
    ))
}

/// Set the auth cookie to an invalid value and set its max age to zero,
/// which should delete the cookie on the client side.
pub fn invalidate_auth_cookie(jar: PrivateCookieJar) -> PrivateCookieJar {
    jar.add(
        Cookie::build((COOKIE_TOKEN, "deleted"))
            .expires(OffsetDateTime::UNIX_EPOCH)
            .max_age(Duration::ZERO)
            .http_only(true)
            .same_site(SameSite::Strict)
            .secure(true)
            // Explicitly set path to root to avoid issues with subpaths.
            // For example, if the cookie is invalidated from a subpath such
            // as 'http://example.com/api/logout', the browser will not
            // invalidate the cookie for 'http://example.com/path1'.
            .path("/"),
    )
}

/// Extract the token from the token cookie.
///
/// # Errors
///
/// Returns a:
/// - [Error::CookieMissing] if the token cookie is not in the cookie jar.
/// - [Error::JSONSerializationError] if the token cannot be deserialized correctly.
pub fn get_token_from_cookies(jar: &PrivateCookieJar) -> Result<Token, Error> {
    match jar.get(COOKIE_TOKEN) {
        Some(token_cookie) => serde_json::from_str(token_cookie.value_trimmed())
            .map_err(|error| Error::JSONSerializationError(error.to_string())),
        _ => Err(Error::CookieMissing),
    }
}

#[cfg(test)]
mod tests {

    use axum_extra::extract::{
        PrivateCookieJar,
        cookie::{Key, SameSite},
    };
    use sha2::{Digest, Sha512};
    use time::{Duration, OffsetDateTime};
    use uuid::Uuid;

    use crate::{
        Error,
        auth::{COOKIE_TOKEN, SessionId, invalidate_auth_cookie, set_auth_cookie},
    };

    use super::get_token_from_cookies;

    #[test]
    fn can_set_cookie() {
        let session_id = Uuid::new_v4();
        let jar = get_jar();

        let jar = set_auth_cookie(
            jar,
            session_id,
            OffsetDateTime::now_utc() + Duration::hours(24),
        )
        .unwrap();

        assert_auth_cookies_eq(jar, session_id);
    }

    #[test]
    fn can_get_token_from_cookies() {
        let session_id = Uuid::new_v4();
        let jar = set_auth_cookie(
            get_jar(),
            session_id,
            OffsetDateTime::now_utc() + Duration::hours(24),
        )
        .unwrap();

        let actual = get_token_from_cookies(&jar).unwrap();

        assert_eq!(actual.session_id, session_id);
    }

    #[test]
    fn invalidate_auth_cookie_succeeds() {
        let session_id = Uuid::new_v4();
        let jar = set_auth_cookie(
            get_jar(),
            session_id,
            OffsetDateTime::now_utc() + Duration::hours(24),
        )
        .unwrap();

        let jar = invalidate_auth_cookie(jar);
        let cookie = jar.get(COOKIE_TOKEN).unwrap();

        assert_eq!(cookie.value(), "deleted");
        assert_eq!(cookie.expires_datetime(), Some(OffsetDateTime::UNIX_EPOCH));
        assert_eq!(cookie.max_age(), Some(Duration::ZERO));

        matches!(
            get_token_from_cookies(&jar),
            Err(Error::JSONSerializationError(_)),
        );
    }

    fn get_jar() -> PrivateCookieJar {
        let hash = Sha512::digest(b"foobar");
        let key = Key::from(&hash);

        PrivateCookieJar::new(key)
    }

    #[track_caller]
    fn assert_auth_cookies_eq(jar: PrivateCookieJar, expected_session_id: SessionId) {
        let token_cookie = jar.get(COOKIE_TOKEN).unwrap();
        let actual_token = get_token_from_cookies(&jar).unwrap();

        assert_eq!(actual_token.session_id, expected_session_id);
        assert_eq!(token_cookie.http_only(), Some(true));
        assert_eq!(token_cookie.same_site(), Some(SameSite::Strict));
        assert_eq!(token_cookie.secure(), Some(true));
        assert_eq!(token_cookie.path(), Some("/"));
    }
}
