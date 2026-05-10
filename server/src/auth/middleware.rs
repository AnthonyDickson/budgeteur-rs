//! Authentication middleware that validates cookies, verifies sessions, and handles redirects.

use axum::{
    extract::{FromRef, FromRequestParts, Request, State},
    middleware::Next,
    response::{IntoResponse, Redirect, Response},
};
use axum_extra::extract::{PrivateCookieJar, cookie::Key};
use axum_htmx::HxRedirect;
use kameo::actor::ActorRef;
use time::UtcDateTime;

use crate::{
    AppState,
    auth::{
        SessionStore, build_log_in_redirect_url, cookie::get_token_from_cookies,
        redirect::build_log_in_redirect_url_from_target, session::Extend,
    },
    endpoints,
};

/// The state needed for the auth middleware
#[derive(Clone)]
pub struct AuthState {
    /// The key to be used for signing and encrypting private cookies.
    pub cookie_key: Key,
    /// An in-memory store for managing sessions
    pub session_actor: ActorRef<SessionStore>,
}

impl FromRef<AppState> for AuthState {
    fn from_ref(state: &AppState) -> Self {
        Self {
            cookie_key: state.cookie_key.clone(),
            session_actor: state.session_actor.clone(),
        }
    }
}

// this impl tells `PrivateCookieJar` how to access the key from our state
impl FromRef<AuthState> for Key {
    fn from_ref(state: &AuthState) -> Self {
        state.cookie_key.clone()
    }
}

/// Middleware function that checks for a valid authorization cookie and
/// verifies the session via the session actor.
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

    let (mut parts, body) = request.into_parts();
    let jar = match PrivateCookieJar::from_request_parts(&mut parts, &state).await {
        Ok(jar) => jar,
        Err(err) => {
            tracing::error!("Error getting cookie jar: {err:?}. Redirecting to log in page.");
            return get_redirect(&log_in_redirect_url);
        }
    };
    let token = match get_token_from_cookies(&jar) {
        Ok(token) => token,
        Err(_) => return get_redirect(&log_in_redirect_url),
    };

    match state
        .session_actor
        .ask(Extend {
            id: token.session_id,
            now: UtcDateTime::now(),
        })
        .await
    {
        Ok(Some(_session)) => {
            let request = Request::from_parts(parts, body);
            next.run(request).await
        }
        Ok(None) => {
            tracing::debug!("Session expired or missing. Redirecting to log in.");
            get_redirect(&log_in_redirect_url)
        }
        Err(err) => {
            tracing::error!("Error communicating with session actor: {err:?}");
            get_redirect(&log_in_redirect_url)
        }
    }
}

/// Middleware function that checks for a valid authorization cookie.
/// The request is executed normally if the session is valid, otherwise a
/// redirect to the log-in page is returned.
pub async fn auth_guard(State(state): State<AuthState>, request: Request, next: Next) -> Response {
    auth_guard_internal(state, request, next, |redirect_url| {
        Redirect::to(redirect_url).into_response()
    })
    .await
}

/// Middleware function that checks for a valid authorization cookie.
/// The request is executed normally if the session is valid, otherwise an
/// HTMX redirect to the log-in page is returned.
pub async fn auth_guard_hx(
    State(state): State<AuthState>,
    request: Request,
    next: Next,
) -> Response {
    auth_guard_internal(state, request, next, |redirect_url| {
        (
            HxRedirect(redirect_url.to_owned()),
            axum::http::StatusCode::OK,
        )
            .into_response()
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
        cookie::{Cookie, Key},
    };
    use axum_test::TestServer;
    use kameo::actor::Spawn;
    use sha2::Digest;
    use time::{Duration, OffsetDateTime, UtcDateTime};

    use crate::{
        Error,
        auth::{
            COOKIE_TOKEN, SessionStore, auth_guard, auth_guard_hx,
            middleware::AuthState,
            session::{Session, Set},
            set_auth_cookie,
        },
        endpoints::{self, format_endpoint},
    };

    async fn test_handler() -> Html<&'static str> {
        Html("<h1>Hello, World!</h1>")
    }

    async fn stub_log_in_route(
        State(state): State<AuthState>,
        jar: PrivateCookieJar,
    ) -> Result<PrivateCookieJar, Error> {
        let session = Session::new(UtcDateTime::now());

        state
            .session_actor
            .tell(Set {
                session: session.clone(),
            })
            .await
            .map_err(|err| {
                Error::JSONSerializationError(format!(
                    "Could not communicate with session actor: {err}"
                ))
            })?;

        set_auth_cookie(
            jar,
            session.id,
            OffsetDateTime::now_utc() + Duration::hours(24),
        )
    }

    const TEST_LOG_IN_ROUTE_PATH: &str = "/log_in/{user_id}";
    const TEST_PROTECTED_ROUTE: &str = "/protected";
    const TEST_API_ROUTE: &str = "/api/protected";

    fn get_test_server() -> TestServer {
        let hash = sha2::Sha512::digest("nafstenoas");
        let session_actor = SessionStore::spawn(SessionStore::new());
        let state = AuthState {
            cookie_key: Key::from(&hash),
            session_actor,
        };

        let app = Router::new()
            .route(TEST_PROTECTED_ROUTE, get(test_handler))
            .route_layer(middleware::from_fn_with_state(state.clone(), auth_guard))
            .route(TEST_LOG_IN_ROUTE_PATH, post(stub_log_in_route))
            .with_state(state.clone());

        TestServer::new(app)
    }

    fn get_test_server_hx() -> TestServer {
        let hash = sha2::Sha512::digest("nafstenoas");
        let session_actor = SessionStore::spawn(SessionStore::new());
        let state = AuthState {
            cookie_key: Key::from(&hash),
            session_actor,
        };

        let app = Router::new()
            .route(TEST_API_ROUTE, get(test_handler))
            .route_layer(middleware::from_fn_with_state(state.clone(), auth_guard_hx))
            .with_state(state.clone());

        TestServer::new(app)
    }

    #[tokio::test]
    async fn get_protected_route_with_valid_cookie() {
        let server = get_test_server();
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
    async fn protected_route_with_valid_cookie_returns_ok() {
        let server = get_test_server();
        let response = server
            .post(&format_endpoint(TEST_LOG_IN_ROUTE_PATH, 1))
            .await;

        response.assert_status_ok();
        let jar = response.cookies();

        server
            .get(TEST_PROTECTED_ROUTE)
            .add_cookies(jar)
            .await
            .assert_status_ok();
    }

    #[tokio::test]
    async fn get_protected_route_with_no_auth_cookie_redirects_to_log_in() {
        let server = get_test_server();
        let response = server.get(TEST_PROTECTED_ROUTE).await;

        response.assert_status_see_other();
        let expected_query =
            serde_urlencoded::to_string([("redirect_url", TEST_PROTECTED_ROUTE)]).unwrap();
        let expected_location = format!("{}?{}", endpoints::LOG_IN_VIEW, expected_query);
        assert_eq!(response.header("location"), expected_location);
    }

    #[tokio::test]
    async fn get_protected_route_with_invalid_auth_cookie_redirects_to_log_in() {
        let server = get_test_server();
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
        let server = get_test_server();
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
        let server = get_test_server_hx();
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
