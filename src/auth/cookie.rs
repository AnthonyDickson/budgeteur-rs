//! Defines functions for handling user authentication with cookies.

use std::cmp::max;

use axum_extra::extract::{
    PrivateCookieJar,
    cookie::{Cookie, SameSite},
};
use time::{Duration, OffsetDateTime, UtcOffset};

use crate::{Error, auth::token::Token, user::UserID};

/// The name of the cookie that will hold the auth token.
pub const COOKIE_TOKEN: &str = "auth_token";
/// The default duration for which auth cookies are valid.
pub const DEFAULT_COOKIE_DURATION: Duration = Duration::minutes(5);

/// Add an auth token to the cookie jar, indicating that a user is logged in and authenticated.
///
/// Sets the initial expiry of the cookie to `duration` from the current time.
/// You can use [COOKIE_DURATION] for the default duration.
///
/// Returns the cookie jar with the cookie added.
///
/// # Errors
///
/// Returns a [Error::JSONSerializationError] if the expiry time cannot be formatted.
pub fn set_auth_cookie(
    jar: PrivateCookieJar,
    user_id: UserID,
    duration: Duration,
    local_timezone: UtcOffset,
) -> Result<PrivateCookieJar, Error> {
    let expiry = OffsetDateTime::now_utc().to_offset(local_timezone) + duration;

    let token = {
        let token = Token {
            user_id,
            expires_at: expiry,
        };

        serde_json::to_string(&token)
            .map_err(|err| Error::JSONSerializationError(err.to_string()))?
    };

    Ok(jar.add(
        Cookie::build((COOKIE_TOKEN, token))
            .expires(expiry)
            .http_only(true)
            .same_site(SameSite::Strict)
            .secure(true)
            // Explicitly set path to root to avoid issues with subpaths. For example, if the
            // cookie is set from a subpath such as 'http://example.com/path1/path2', the
            // browser will not send the cookie in requests for parent paths such as
            // 'http://example.com/path1'.
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

/// Set the expiry of the auth cookie in `jar` to the latest of UTC now
/// plus `duration` and the cookie's expiry.
///
/// # Errors
///
/// The cookie jar is not modified if an error is returned.
///
/// Returns:
/// - [Error::CookieMissing] if the auth cookie or expiry cookie are not in the cookie jar.
/// - [Error::InvalidDateFormat] if the new expiry date time cannot be formatted.
///
/// # Panics
///
/// Panics if adding `duration` to the current time overflows.
/// See [time::Date::MAX] for more information.
pub fn extend_auth_cookie_duration_if_needed(
    jar: PrivateCookieJar,
    duration: Duration,
    local_timezone: UtcOffset,
) -> Result<PrivateCookieJar, Error> {
    let token = get_token_from_cookies(&jar)?;
    let new_expiry = OffsetDateTime::now_utc().to_offset(local_timezone) + duration;
    let expiry = max(token.expires_at, new_expiry);

    set_auth_cookie_expiry(jar, expiry)
}

/// Sets the expires field of the auth cookie and the expires field and
/// value of the expiry cookie in `jar` to `expiry`.
///
/// # Errors
///
/// If an error is returned, the cookie jar is not modified.
///
/// Returns a:
/// - [Error::CookieMissing] if the auth cookie or expiry cookie are not in the cookie jar.
/// - [Error::InvalidDateFormat] if the new expiry date time cannot be formatted.
pub fn set_auth_cookie_expiry(
    jar: PrivateCookieJar,
    expiry: OffsetDateTime,
) -> Result<PrivateCookieJar, Error> {
    let mut token_cookie = jar.get(COOKIE_TOKEN).ok_or(Error::CookieMissing)?;

    let token: Token = serde_json::from_str(token_cookie.value_trimmed())
        .map_err(|err| Error::JSONSerializationError(err.to_string()))?;

    let updated_token_string = serde_json::to_string(&Token {
        user_id: token.user_id,
        expires_at: expiry,
    })
    .map_err(|err| Error::JSONSerializationError(err.to_string()))?;

    token_cookie.set_expires(expiry);
    token_cookie.set_value(updated_token_string);

    // Need to set the secure, http_only, and same_site flags again since they
    // are not set in requests from clients (clients only send the
    // key-value pair).
    token_cookie.set_http_only(true);
    token_cookie.set_same_site(SameSite::Strict);
    token_cookie.set_secure(true);
    token_cookie.set_path("/");

    Ok(jar.add(token_cookie))
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
    use time::{Duration, OffsetDateTime, UtcOffset};

    use crate::{
        Error,
        auth::{
            COOKIE_TOKEN,
            cookie::{
                DEFAULT_COOKIE_DURATION, extend_auth_cookie_duration_if_needed,
                get_token_from_cookies, set_auth_cookie_expiry,
            },
            invalidate_auth_cookie, set_auth_cookie,
            token::Token,
        },
        user::UserID,
    };

    #[test]
    fn can_set_cookie() {
        let expected_token = Token {
            user_id: UserID::new(1),
            expires_at: OffsetDateTime::now_utc() + DEFAULT_COOKIE_DURATION,
        };
        let jar = get_jar();

        let jar = set_auth_cookie(
            jar,
            expected_token.user_id,
            DEFAULT_COOKIE_DURATION,
            UtcOffset::UTC,
        )
        .unwrap();

        assert_auth_cookies_eq(jar, &expected_token);
    }

    #[test]
    fn can_get_token_from_cookies() {
        let expected = Token {
            user_id: UserID::new(1),
            expires_at: OffsetDateTime::now_utc() + DEFAULT_COOKIE_DURATION,
        };
        let jar = set_auth_cookie(
            get_jar(),
            expected.user_id,
            DEFAULT_COOKIE_DURATION,
            UtcOffset::UTC,
        )
        .unwrap();

        let actual = get_token_from_cookies(&jar).unwrap();

        assert_eq!(actual.user_id, expected.user_id);
        assert_date_time_close(expected.expires_at, actual.expires_at);
    }

    #[test]
    fn can_set_cookie_expires() {
        let expected = Token {
            user_id: UserID::new(1),
            expires_at: OffsetDateTime::now_utc() + Duration::days(10),
        };
        let jar = get_jar();
        let jar = set_auth_cookie(
            jar,
            expected.user_id,
            DEFAULT_COOKIE_DURATION,
            UtcOffset::UTC,
        )
        .unwrap();

        let updated_jar = set_auth_cookie_expiry(jar, expected.expires_at).unwrap();

        assert_auth_cookies_eq(updated_jar, &expected);
    }

    #[test]
    fn can_extend_cookie_duration() {
        let user_id = UserID::new(1);
        let jar = get_jar();
        let jar = set_auth_cookie(jar, user_id, DEFAULT_COOKIE_DURATION, UtcOffset::UTC).unwrap();
        let initial_token = get_token_from_cookies(&jar).unwrap();
        let expected = Token {
            user_id,
            expires_at: initial_token.expires_at + DEFAULT_COOKIE_DURATION,
        };

        let jar = extend_auth_cookie_duration_if_needed(jar, Duration::minutes(10), UtcOffset::UTC)
            .unwrap();

        assert_auth_cookies_eq(jar, &expected);
    }

    #[test]
    fn sets_secure_httponly_samesite_flags_if_missing() {
        let user_id = UserID::new(1);
        let jar = get_jar();
        let jar = set_auth_cookie(jar, user_id, DEFAULT_COOKIE_DURATION, UtcOffset::UTC).unwrap();
        let initial_token = get_token_from_cookies(&jar).unwrap();
        let expected = Token {
            user_id,
            expires_at: initial_token.expires_at + DEFAULT_COOKIE_DURATION,
        };
        for mut cookie in jar.iter() {
            cookie.set_secure(false);
            cookie.set_http_only(false);
            cookie.set_same_site(SameSite::None);
        }

        let jar = extend_auth_cookie_duration_if_needed(jar, Duration::minutes(10), UtcOffset::UTC)
            .unwrap();

        assert_auth_cookies_eq(jar, &expected);
    }

    #[test]
    fn cookie_duration_does_not_change() {
        let expected = Token {
            user_id: UserID::new(1),
            expires_at: OffsetDateTime::now_utc() + DEFAULT_COOKIE_DURATION,
        };
        let jar = set_auth_cookie(
            get_jar(),
            expected.user_id,
            DEFAULT_COOKIE_DURATION,
            UtcOffset::UTC,
        )
        .unwrap();
        let stale_cookie = jar.get(COOKIE_TOKEN).unwrap();
        let want = Some(stale_cookie.expires_datetime().unwrap());

        // The initial cookie is set to expire in 5 minutes, so extending it by 5 seconds should not change the expiry.
        let jar = extend_auth_cookie_duration_if_needed(jar, Duration::seconds(5), UtcOffset::UTC)
            .unwrap();

        let cookie = jar.get(COOKIE_TOKEN).unwrap();
        assert_eq!(cookie.expires_datetime(), want);
    }

    #[test]
    fn invalidate_auth_cookie_succeeds() {
        let user_id = UserID::new(1);
        let jar =
            set_auth_cookie(get_jar(), user_id, DEFAULT_COOKIE_DURATION, UtcOffset::UTC).unwrap();

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
    fn assert_date_time_close(expected: OffsetDateTime, actual: OffsetDateTime) {
        assert!(
            (expected - actual).abs() < Duration::seconds(1),
            "got date time {:?}, want {:?}",
            actual,
            expected
        );
    }

    #[track_caller]
    fn assert_auth_cookies_eq(jar: PrivateCookieJar, expected_token: &Token) {
        let token_cookie = jar.get(COOKIE_TOKEN).unwrap();
        let actual_token = get_token_from_cookies(&jar).unwrap();

        assert_eq!(actual_token.user_id, expected_token.user_id);
        assert_date_time_close(actual_token.expires_at, expected_token.expires_at);
        assert_date_time_close(
            token_cookie.expires_datetime().unwrap(),
            expected_token.expires_at,
        );
        assert_eq!(token_cookie.http_only(), Some(true));
        assert_eq!(token_cookie.same_site(), Some(SameSite::Strict));
        assert_eq!(token_cookie.secure(), Some(true));
        assert_eq!(token_cookie.path(), Some("/"));
    }
}
