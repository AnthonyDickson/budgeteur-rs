//! Authentication middleware that validates cookies, extends sessions, and handles redirects.

use axum::{
    extract::{FromRef, FromRequestParts, Request, State},
    http::{StatusCode, header::SET_COOKIE},
    middleware::Next,
    response::{IntoResponse, Redirect, Response},
};
use axum_extra::extract::{PrivateCookieJar, cookie::Key};
use axum_htmx::HxRedirect;
use time::Duration;

use crate::{
    AppState,
    auth::{
        build_log_in_redirect_url,
        cookie::{extend_auth_cookie_duration_if_needed, get_token_from_cookies},
        redirect::build_log_in_redirect_url_from_target,
    },
    endpoints,
    timezone::get_local_offset,
};

/// The state needed for the auth middleware
#[derive(Clone)]
pub struct AuthState {
    /// The key to be used for signing and encrypting private cookies.
    pub cookie_key: Key,
    /// The duration for which cookies used for authentication are valid.
    pub cookie_duration: Duration,
    /// The local timezone as a canonical timezone name, e.g. "Pacific/Auckland".
    pub local_timezone: String,
}

impl FromRef<AppState> for AuthState {
    fn from_ref(state: &AppState) -> Self {
        Self {
            cookie_key: state.cookie_key.clone(),
            cookie_duration: state.cookie_duration,
            local_timezone: state.local_timezone.clone(),
        }
    }
}

// this impl tells `PrivateCookieJar` how to access the key from our state
impl FromRef<AuthState> for Key {
    fn from_ref(state: &AuthState) -> Self {
        state.cookie_key.clone()
    }
}

/// Middleware function that checks for a valid authorization cookie.
/// The user ID is placed into request and then the request executed normally if the cookie is valid, otherwise a redirect to the log-in page is returned using `get_redirect`.
///
/// **Note**: Route handlers can use the function argument `Extension(user_id): Extension<UserID>` to receive the user ID.
///
/// **Note**: The app state must contain an `axum_extra::extract::cookie::Key` for decrypting and verifying the cookie contents.
#[inline]
async fn auth_guard_internal(
    state: AuthState,
    request: Request,
    next: Next,
    get_redirect: impl Fn(&str) -> Response,
) -> Response {
    let log_in_redirect_url = build_log_in_redirect_url(&request).unwrap_or_else(|| {
        if request.uri().path().starts_with("/api") {
            tracing::warn!(
                "Missing or invalid HTMX headers for /api request. Falling back to dashboard."
            );
        } else {
            tracing::warn!("Invalid redirect URL from request URI. Falling back to dashboard.");
        }

        build_log_in_redirect_url_from_target(endpoints::DASHBOARD_VIEW)
            .unwrap_or_else(|| endpoints::LOG_IN_VIEW.to_owned())
    });
    let local_offset = match get_local_offset(&state.local_timezone) {
        Some(offset) => offset,
        None => {
            tracing::error!("Error getting local timezone. Redirecting to log in page.");
            return get_redirect(&log_in_redirect_url);
        }
    };

    let (mut parts, body) = request.into_parts();
    let jar = match PrivateCookieJar::from_request_parts(&mut parts, &state).await {
        Ok(jar) => jar,
        Err(err) => {
            tracing::error!("Error getting cookie jar: {err:?}. Redirecting to log in page.");
            return get_redirect(&log_in_redirect_url);
        }
    };
    let user_id = match get_token_from_cookies(&jar) {
        Ok(token) => token.user_id,
        Err(_) => return get_redirect(&log_in_redirect_url),
    };

    parts.extensions.insert(user_id);
    let request = Request::from_parts(parts, body);
    let response = next.run(request).await;

    let (mut parts, body) = response.into_parts();
    let jar = match extend_auth_cookie_duration_if_needed(
        jar.clone(),
        Duration::minutes(5),
        local_offset,
    ) {
        Ok(updated_jar) => updated_jar,
        Err(err) => {
            tracing::error!("Error extending cookie duration: {err:?}. Rolling back cookie jar.");
            jar
        }
    };
    for (key, val) in jar.into_response().headers().iter() {
        if key != SET_COOKIE {
            continue;
        }

        parts.headers.append(key, val.to_owned());
    }

    Response::from_parts(parts, body)
}

/// Middleware function that checks for a valid authorization cookie.
/// The user ID is placed into request and then the request executed normally if the cookie is valid, otherwise a redirect to the log-in page is returned.
///
/// **Note**: Route handlers can use the function argument `Extension(user_id): Extension<UserID>` to receive the user ID.
///
/// **Note**: The app state must contain an `axum_extra::extract::cookie::Key` for decrypting and verifying the cookie contents.
pub async fn auth_guard(State(state): State<AuthState>, request: Request, next: Next) -> Response {
    auth_guard_internal(state, request, next, |redirect_url| {
        Redirect::to(redirect_url).into_response()
    })
    .await
}

/// Middleware function that checks for a valid authorization cookie.
/// The user ID is placed into request and then the request executed normally if the cookie is valid, otherwise a HTMX redirect to the log-in page is returned.
///
/// **Note**: Route handlers can use the function argument `Extension(user_id): Extension<UserID>` to receive the user ID.
///
/// **Note**: The app state must contain an `axum_extra::extract::cookie::Key` for decrypting and verifying the cookie contents.
pub async fn auth_guard_hx(
    State(state): State<AuthState>,
    request: Request,
    next: Next,
) -> Response {
    auth_guard_internal(state, request, next, |redirect_url| {
        (HxRedirect(redirect_url.to_owned()), StatusCode::OK).into_response()
    })
    .await
}

#[cfg(test)]
mod auth_guard_tests {
    use axum::{
        Router,
        extract::State,
        middleware,
        response::Html,
        routing::{get, post},
    };
    use axum_extra::extract::{
        PrivateCookieJar,
        cookie::{Cookie, Key, SameSite},
    };
    use axum_test::TestServer;
    use sha2::Digest;
    use time::{Duration, OffsetDateTime};

    use crate::{
        Error,
        auth::{
            AuthState, COOKIE_TOKEN, DEFAULT_COOKIE_DURATION, UserID, auth_guard, auth_guard_hx,
            set_auth_cookie,
        },
        endpoints::{self, format_endpoint},
        timezone::get_local_offset,
    };

    async fn test_handler() -> Html<&'static str> {
        Html("<h1>Hello, World!</h1>")
    }

    async fn stub_log_in_route(
        State(state): State<AuthState>,
        jar: PrivateCookieJar,
    ) -> Result<PrivateCookieJar, Error> {
        let local_timezone = get_local_offset(&state.local_timezone).unwrap();

        set_auth_cookie(jar, UserID::new(1), state.cookie_duration, local_timezone)
    }

    const TEST_LOG_IN_ROUTE_PATH: &str = "/log_in/{user_id}";
    const TEST_PROTECTED_ROUTE: &str = "/protected";
    const TEST_API_ROUTE: &str = "/api/protected";

    fn get_test_server(cookie_duration: Duration) -> TestServer {
        let hash = sha2::Sha512::digest("nafstenoas");
        let state = AuthState {
            cookie_key: Key::from(&hash),
            cookie_duration,
            local_timezone: "Etc/UTC".to_owned(),
        };

        let app = Router::new()
            .route(TEST_PROTECTED_ROUTE, get(test_handler))
            .route_layer(middleware::from_fn_with_state(state.clone(), auth_guard))
            .route(TEST_LOG_IN_ROUTE_PATH, post(stub_log_in_route))
            .with_state(state.clone());

        TestServer::new(app).expect("Could not create test server.")
    }

    fn get_test_server_hx(cookie_duration: Duration) -> TestServer {
        let hash = sha2::Sha512::digest("nafstenoas");
        let state = AuthState {
            cookie_key: Key::from(&hash),
            cookie_duration,
            local_timezone: "Etc/UTC".to_owned(),
        };

        let app = Router::new()
            .route(TEST_API_ROUTE, get(test_handler))
            .route_layer(middleware::from_fn_with_state(state.clone(), auth_guard_hx))
            .with_state(state.clone());

        TestServer::new(app).expect("Could not create test server.")
    }

    #[tokio::test]
    async fn get_protected_route_with_valid_cookie() {
        let server = get_test_server(DEFAULT_COOKIE_DURATION);
        let response = server
            .post(&format_endpoint(TEST_LOG_IN_ROUTE_PATH, 1))
            .await;

        response.assert_status_ok();
        let token_cookie = response.cookie(COOKIE_TOKEN);

        server
            .get(TEST_PROTECTED_ROUTE)
            .add_cookie(token_cookie)
            .await
            .assert_status_ok();
    }

    #[tokio::test]
    async fn auth_guard_sets_auth_and_expiry_cookies() {
        let server = get_test_server(DEFAULT_COOKIE_DURATION);
        let response = server
            .post(&format_endpoint(TEST_LOG_IN_ROUTE_PATH, 1))
            .await;

        response.assert_status_ok();
        let jar = response.cookies();

        let response = server.get(TEST_PROTECTED_ROUTE).add_cookies(jar).await;
        let jar = response.cookies();
        assert!(
            jar.get(COOKIE_TOKEN).is_some(),
            "expected token cookie to be set by auth guard"
        );
    }

    #[track_caller]
    fn assert_date_time_close(left: OffsetDateTime, right: OffsetDateTime) {
        assert!(
            (left - right).abs() < Duration::seconds(1),
            "got date time {:?}, want {:?}",
            left,
            right
        );
    }

    #[tokio::test]
    async fn auth_guard_extends_valid_cookie_duration() {
        let server = get_test_server(Duration::seconds(5));
        let response = server
            .post(&format_endpoint(TEST_LOG_IN_ROUTE_PATH, 1))
            .await;

        response.assert_status_ok();
        let response_time = OffsetDateTime::now_utc();
        let jar = response.cookies();
        assert_date_time_close(
            jar.get(COOKIE_TOKEN).unwrap().expires_datetime().unwrap(),
            response_time + Duration::seconds(5),
        );

        let response = server.get(TEST_PROTECTED_ROUTE).add_cookies(jar).await;

        let auth_cookie = response.cookie(COOKIE_TOKEN);
        assert_date_time_close(
            auth_cookie.expires_datetime().unwrap(),
            response_time + Duration::minutes(5),
        );
        assert_eq!(auth_cookie.secure(), Some(true));
        assert_eq!(auth_cookie.http_only(), Some(true));
        assert_eq!(auth_cookie.same_site(), Some(SameSite::Strict));
    }

    #[tokio::test]
    async fn get_protected_route_with_no_auth_cookie_redirects_to_log_in() {
        let server = get_test_server(DEFAULT_COOKIE_DURATION);
        let response = server.get(TEST_PROTECTED_ROUTE).await;

        response.assert_status_see_other();
        let expected_query =
            serde_urlencoded::to_string([("redirect_url", TEST_PROTECTED_ROUTE)]).unwrap();
        let expected_location = format!("{}?{}", endpoints::LOG_IN_VIEW, expected_query);
        assert_eq!(response.header("location"), expected_location);
    }

    #[tokio::test]
    async fn get_protected_route_with_invalid_auth_cookie_redirects_to_log_in() {
        let server = get_test_server(DEFAULT_COOKIE_DURATION);
        let response = server
            .get(TEST_PROTECTED_ROUTE)
            .add_cookie(Cookie::build((COOKIE_TOKEN, "FOOBAR")).build())
            .await;

        response.assert_status_see_other();
        let expected_query =
            serde_urlencoded::to_string([("redirect_url", TEST_PROTECTED_ROUTE)]).unwrap();
        let expected_location = format!("{}?{}", endpoints::LOG_IN_VIEW, expected_query);
        assert_eq!(response.header("location"), expected_location);
    }

    #[tokio::test]
    async fn get_protected_route_with_expired_auth_cookie_redirects_to_log_in() {
        let server = get_test_server(DEFAULT_COOKIE_DURATION);
        let response = server
            .post(&format_endpoint(TEST_LOG_IN_ROUTE_PATH, 1))
            .await;

        response.assert_status_ok();
        let mut token_cookie = response.cookie(COOKIE_TOKEN);
        token_cookie.set_expires(OffsetDateTime::UNIX_EPOCH);

        let response = server
            .get(TEST_PROTECTED_ROUTE)
            .add_cookie(token_cookie)
            .await;

        response.assert_status_see_other();
        let expected_query =
            serde_urlencoded::to_string([("redirect_url", TEST_PROTECTED_ROUTE)]).unwrap();
        let expected_location = format!("{}?{}", endpoints::LOG_IN_VIEW, expected_query);
        assert_eq!(response.header("location"), expected_location);
    }

    #[tokio::test]
    async fn api_route_uses_hx_current_url_for_redirect() {
        let server = get_test_server_hx(DEFAULT_COOKIE_DURATION);
        let current_url = "/transactions?range=month&anchor=2025-10-05";
        let response = server
            .get(TEST_API_ROUTE)
            .add_header("HX-Request", "true")
            .add_header("HX-Current-URL", current_url)
            .await;

        response.assert_status_ok();
        let expected_query = serde_urlencoded::to_string([("redirect_url", current_url)]).unwrap();
        let expected_location = format!("{}?{}", endpoints::LOG_IN_VIEW, expected_query);
        assert_eq!(response.header("hx-redirect"), expected_location);
    }
}
