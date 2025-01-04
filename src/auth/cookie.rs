//! Defines functions for handling user authentication with cookies.

use std::{cmp::max, num::ParseIntError};

use axum_extra::extract::{
    cookie::{Cookie, SameSite},
    PrivateCookieJar,
};
use time::{
    format_description::BorrowedFormatItem, macros::format_description, Duration, OffsetDateTime,
};

use crate::models::UserID;

use super::AuthError;

pub(crate) const COOKIE_USER_ID: &str = "user_id";
pub(crate) const COOKIE_EXPIRY: &str = "expiry";
/// The default duration for which auth cookies are valid.
pub(crate) const DEFAULT_COOKIE_DURATION: Duration = Duration::minutes(5);

/// Add an auth cookie to the cookie jar, indicating that a user is logged in and authenticated.
///
/// Sets the initial expiry of the cookie to `duration` from the current time.
/// You can use [COOKIE_DURATION] for the default duration.
///
/// Returns the cookie jar with the cookie added.
///
/// # Errors
///
/// Returns a [time::error::Format] if the expiry time cannot be formatted.
pub(crate) fn set_auth_cookie(
    jar: PrivateCookieJar,
    user_id: UserID,
    duration: Duration,
) -> Result<PrivateCookieJar, time::error::Format> {
    let expiry = OffsetDateTime::now_utc() + duration;
    // Use format instead of to_string to avoid errors at midnight when the hour is printed as
    // a single digit when [DATE_TIME_FORMAT] expects two digits.
    let expiry_string = expiry.format(DATE_TIME_FORMAT)?;

    Ok(jar
        .add(
            Cookie::build((COOKIE_USER_ID, user_id.as_i64().to_string()))
                .expires(expiry)
                .http_only(true)
                .same_site(SameSite::Strict)
                .secure(true),
        )
        .add(
            Cookie::build((COOKIE_EXPIRY, expiry_string))
                .expires(expiry)
                .http_only(true)
                .same_site(SameSite::Strict)
                .secure(true),
        ))
}

/// Set the auth cookie to an invalid value and set its max age to zero, which should delete the cookie on the client side.
pub(crate) fn invalidate_auth_cookie(jar: PrivateCookieJar) -> PrivateCookieJar {
    jar.add(
        Cookie::build((COOKIE_USER_ID, "deleted"))
            .expires(OffsetDateTime::UNIX_EPOCH)
            .max_age(Duration::ZERO)
            .http_only(true)
            .same_site(SameSite::Strict)
            .secure(true),
    )
    .add(
        Cookie::build((COOKIE_EXPIRY, "deleted"))
            .expires(OffsetDateTime::UNIX_EPOCH)
            .max_age(Duration::ZERO)
            .http_only(true)
            .same_site(SameSite::Strict)
            .secure(true),
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
/// - [AuthError::DateError] if the auth cookie or expiry cookie are not in the cookie jar.
/// - [AuthError::DateError] if extending the cookie by `duration` would overflow the date time.
/// - [AuthError::DateError] if the new expiry date time cannot be formatted.
pub(crate) fn extend_auth_cookie_duration_if_needed(
    jar: PrivateCookieJar,
    duration: Duration,
) -> Result<PrivateCookieJar, AuthError> {
    let expiry_cookie = jar.get(COOKIE_EXPIRY).ok_or(AuthError::CookieMissing)?;
    let current_expiry = extract_date_time(&expiry_cookie).map_err(|_| AuthError::DateError)?;

    let new_expiry = OffsetDateTime::now_utc()
        .checked_add(duration)
        .ok_or(AuthError::DateError)?;

    let expiry = max(current_expiry, new_expiry);

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
/// - [AuthError::CookieMissing] if the auth cookie or expiry cookie are not in the cookie jar.
/// - [AuthError::DateError] if the new expiry date time cannot be formatted.
pub(crate) fn set_auth_cookie_expiry(
    jar: PrivateCookieJar,
    expiry: OffsetDateTime,
) -> Result<PrivateCookieJar, AuthError> {
    let expiry_string = expiry
        .format(DATE_TIME_FORMAT)
        .map_err(|_| AuthError::DateError)?;

    let mut auth_cookie = jar.get(COOKIE_USER_ID).ok_or(AuthError::CookieMissing)?;
    let mut expiry_cookie = jar.get(COOKIE_EXPIRY).ok_or(AuthError::CookieMissing)?;

    auth_cookie.set_expires(expiry);
    expiry_cookie.set_expires(expiry);
    expiry_cookie.set_value(expiry_string);

    Ok(jar.add(auth_cookie).add(expiry_cookie))
}

pub(crate) fn get_user_id_from_auth_cookie(jar: &PrivateCookieJar) -> Result<UserID, AuthError> {
    match jar.get(COOKIE_USER_ID) {
        Some(user_id_cookie) => {
            extract_user_id(&user_id_cookie).map_err(|_| AuthError::InvalidCredentials)
        }
        _ => Err(AuthError::InvalidCredentials),
    }
}

/// Date time format for the cookie expiry, e.g. "2021-01-01 00:00:00.000000 +00:00:00".
const DATE_TIME_FORMAT: &[BorrowedFormatItem] = format_description!(
    "[year]-[month]-[day] [hour]:[minute]:[second].[subsecond] [offset_hour \
         sign:mandatory]:[offset_minute]:[offset_second]"
);

pub(crate) fn extract_date_time(cookie: &Cookie) -> Result<OffsetDateTime, time::error::Parse> {
    OffsetDateTime::parse(cookie.value_trimmed(), DATE_TIME_FORMAT)
}

pub(crate) fn extract_user_id(cookie: &Cookie) -> Result<UserID, ParseIntError> {
    let id: i64 = cookie.value_trimmed().parse()?;

    Ok(UserID::new(id))
}

#[cfg(test)]
mod cookie_tests {

    use axum_extra::extract::{
        cookie::{Cookie, Key},
        PrivateCookieJar,
    };
    use sha2::{Digest, Sha512};
    use time::{macros::datetime, Duration, OffsetDateTime, UtcOffset};

    use crate::{
        auth::{
            cookie::{
                extract_date_time, extract_user_id, get_user_id_from_auth_cookie, COOKIE_EXPIRY,
                COOKIE_USER_ID, DATE_TIME_FORMAT, DEFAULT_COOKIE_DURATION,
            },
            AuthError,
        },
        models::UserID,
    };

    use super::{
        extend_auth_cookie_duration_if_needed, invalidate_auth_cookie, set_auth_cookie,
        set_auth_cookie_expiry,
    };

    #[test]
    fn can_extract_date_time() {
        let want = OffsetDateTime::now_utc() + Duration::minutes(5);
        let date_time_string = want.format(DATE_TIME_FORMAT).unwrap();
        let cookie = Cookie::build((COOKIE_EXPIRY, date_time_string)).build();

        let got = extract_date_time(&cookie).unwrap();

        assert_eq!(got, want, "got date time {:?}, want {:?}", got, want);
    }

    #[test]
    fn can_extract_date_time_at_midnight() {
        let want = datetime!(2021-01-01 00:00:00).assume_offset(UtcOffset::UTC);
        // Use format instead of to_string to avoid errors at midnight when the hour is printed as
        // a single digit when [DATE_TIME_FORMAT] expects two digits.
        let date_time_string = want.format(DATE_TIME_FORMAT).unwrap();
        let cookie = Cookie::build((COOKIE_EXPIRY, date_time_string)).build();

        let got = extract_date_time(&cookie).unwrap();

        assert_eq!(got, want, "got date time {:?}, want {:?}", got, want);
    }

    #[test]
    fn can_extract_user_id() {
        let user_id = UserID::new(1);
        let cookie = Cookie::build((COOKIE_USER_ID, user_id.as_i64().to_string())).build();

        let got = extract_user_id(&cookie).unwrap();

        assert_eq!(got, user_id);
    }

    fn get_jar() -> PrivateCookieJar {
        let hash = Sha512::digest(b"foobar");
        let key = Key::from(&hash);

        PrivateCookieJar::new(key)
    }

    /// Test helper macro to assert that two date times are within one second
    /// of each other. Used instead of a function so that the file and line
    /// number of the caller is included in the error message instead of the
    /// helper.
    macro_rules! assert_date_time_close {
        ($left:expr, $right:expr) => {
            assert!(
                ($left - $right).abs() < Duration::seconds(1),
                "got date time {:?}, want {:?}",
                $left,
                $right
            );
        };
    }

    #[test]
    fn can_set_cookie() {
        let jar = get_jar();
        let user_id = UserID::new(1);

        let jar = set_auth_cookie(jar, user_id, DEFAULT_COOKIE_DURATION).unwrap();
        let user_id_cookie = jar.get(COOKIE_USER_ID).unwrap();
        let expiry_cookie = jar.get(COOKIE_EXPIRY).unwrap();

        let retrieved_user_id = extract_user_id(&user_id_cookie).unwrap();
        let got_expiry = extract_date_time(&expiry_cookie).unwrap();

        assert_eq!(retrieved_user_id, user_id);
        assert_date_time_close!(got_expiry, OffsetDateTime::now_utc() + Duration::minutes(5));
    }

    #[test]
    fn get_user_id_from_cookie_succeeds() {
        let user_id = UserID::new(1);
        let jar = set_auth_cookie(get_jar(), user_id, DEFAULT_COOKIE_DURATION).unwrap();

        let retrieved_user_id = get_user_id_from_auth_cookie(&jar).unwrap();

        assert_eq!(retrieved_user_id, user_id);
    }

    #[test]
    fn can_set_cookie_expires() {
        let jar = get_jar();
        let jar = set_auth_cookie(jar, UserID::new(1), DEFAULT_COOKIE_DURATION).unwrap();

        let want = OffsetDateTime::now_utc() + Duration::days(10);
        let updated_jar = set_auth_cookie_expiry(jar, want).unwrap();
        let id_cookie = updated_jar.get(COOKIE_USER_ID).unwrap();
        let expiry_cookie = updated_jar.get(COOKIE_EXPIRY).unwrap();

        assert_eq!(id_cookie.expires_datetime().unwrap(), want);
        assert_eq!(expiry_cookie.expires_datetime().unwrap(), want);
        assert_eq!(extract_user_id(&id_cookie).unwrap(), UserID::new(1));
        assert_eq!(extract_date_time(&expiry_cookie).unwrap(), want);
    }

    #[test]
    fn can_extend_cookie_duration() {
        let jar = get_jar();
        let jar = set_auth_cookie(jar, UserID::new(1), DEFAULT_COOKIE_DURATION).unwrap();

        let initial_cookie = jar.get(COOKIE_EXPIRY).unwrap();
        let want = extract_date_time(&initial_cookie)
            .unwrap()
            .checked_add(Duration::minutes(5))
            .unwrap();

        let jar = extend_auth_cookie_duration_if_needed(jar, Duration::minutes(10)).unwrap();
        let got_id_cookie = jar.get(COOKIE_USER_ID).unwrap();
        let got_expiry_cookie = jar.get(COOKIE_EXPIRY).unwrap();
        let expiry_cookie_value = extract_date_time(&got_expiry_cookie).unwrap();

        assert_date_time_close!(expiry_cookie_value, want);
        assert_date_time_close!(got_id_cookie.expires_datetime().unwrap(), want);
        assert_date_time_close!(got_expiry_cookie.expires_datetime().unwrap(), want);
    }

    #[test]
    fn cookie_duration_does_not_change() {
        let user_id = UserID::new(1);
        let jar = set_auth_cookie(get_jar(), user_id, DEFAULT_COOKIE_DURATION).unwrap();
        let stale_cookie = jar.get(COOKIE_USER_ID).unwrap();
        let want = Some(stale_cookie.expires_datetime().unwrap());

        // The initial cookie is set to expire in 5 minutes, so extending it by 5 seconds should not change the expiry.
        let jar = extend_auth_cookie_duration_if_needed(jar, Duration::seconds(5)).unwrap();

        let cookie = jar.get(COOKIE_USER_ID).unwrap();
        assert_eq!(cookie.expires_datetime(), want);
    }

    #[test]
    fn invalidate_auth_cookie_succeeds() {
        let user_id = UserID::new(1);
        let jar = set_auth_cookie(get_jar(), user_id, DEFAULT_COOKIE_DURATION).unwrap();

        let jar = invalidate_auth_cookie(jar);
        let cookie = jar.get(COOKIE_USER_ID).unwrap();

        assert_eq!(cookie.value(), "deleted");
        assert_eq!(cookie.expires_datetime(), Some(OffsetDateTime::UNIX_EPOCH));
        assert_eq!(cookie.max_age(), Some(Duration::ZERO));

        assert_eq!(
            get_user_id_from_auth_cookie(&jar),
            Err(AuthError::InvalidCredentials),
        );
    }
}
