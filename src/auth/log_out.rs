//! Log-out route handler that invalidates sessions and clears authentication cookies.

use axum::{
    extract::{FromRef, State},
    response::{IntoResponse, Redirect, Response},
};
use axum_extra::extract::PrivateCookieJar;
use kameo::actor::ActorRef;

use crate::{
    AppState,
    auth::{SessionStore, cookie::get_token_from_cookies, invalidate_auth_cookie, session::Delete},
    endpoints,
};

/// The state needed for logging out.
#[derive(Debug, Clone)]
pub struct LogoutState {
    /// An in-memory store for managing sessions
    pub session_actor: ActorRef<SessionStore>,
}

impl FromRef<AppState> for LogoutState {
    fn from_ref(state: &AppState) -> Self {
        Self {
            session_actor: state.session_actor.clone(),
        }
    }
}

/// Invalidate the session and auth cookie, then redirect to the log-in page.
pub async fn get_log_out(State(state): State<LogoutState>, jar: PrivateCookieJar) -> Response {
    if let Ok(token) = get_token_from_cookies(&jar)
        && let Err(err) = state
            .session_actor
            .tell(Delete {
                id: token.session_id,
            })
            .await
    {
        tracing::error!("Error deleting session: {err}");
    }

    let jar = invalidate_auth_cookie(jar);

    (jar, Redirect::to(endpoints::LOG_IN_VIEW)).into_response()
}

#[cfg(test)]
mod log_out_tests {
    use axum::{
        body::Body,
        extract::State,
        http::{Response, StatusCode, header::SET_COOKIE},
    };
    use axum_extra::extract::{
        PrivateCookieJar,
        cookie::{Cookie, Key},
    };
    use kameo::actor::Spawn;
    use sha2::{Digest, Sha512};
    use time::{Duration, OffsetDateTime, UtcDateTime};

    use crate::{
        auth::{
            COOKIE_TOKEN, SessionStore, get_log_out,
            log_out::LogoutState,
            session::{Session, Set},
            set_auth_cookie,
        },
        endpoints,
    };

    #[tokio::test]
    async fn log_out_invalidates_auth_cookie_and_redirects() {
        let session = Session::new(UtcDateTime::now());
        let session_actor = SessionStore::spawn(SessionStore::new());
        session_actor
            .tell(Set {
                session: session.clone(),
            })
            .await
            .unwrap();

        let cookie_jar = set_auth_cookie(
            get_jar(),
            session.id,
            OffsetDateTime::now_utc() + time::Duration::hours(24),
        )
        .unwrap();
        let state = LogoutState { session_actor };

        let response = get_log_out(State(state), cookie_jar).await;

        assert_redirect(&response, endpoints::LOG_IN_VIEW);
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

            if cookie.name() != COOKIE_TOKEN {
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
