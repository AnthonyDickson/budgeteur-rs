use std::collections::HashMap;

use kameo::{
    Actor,
    actor::{ActorRef, Spawn},
    error::SendError,
    prelude::{Context, Message},
};
use kameo_actors::scheduler::{Scheduler, SetInterval};
use time::{Duration, UtcDateTime};
use uuid::Uuid;

const IDLE_TIMEOUT: Duration = Duration::minutes(15);
/// The maximum allowed age for a session.
pub const MAX_SESSION_AGE: Duration = Duration::hours(24);
const CLEAR_EXPIRED_SESSION_INTERVAL: std::time::Duration = std::time::Duration::from_hours(1);

/// A unique identifier for a session.
pub type SessionId = Uuid;

/// A user session with idle timeout and absolute max age.
#[derive(Clone, Debug)]
pub struct Session {
    /// The unique identifier for this session.
    pub id: SessionId,
    /// When the session was created.
    pub issued_at: UtcDateTime,
    /// When the session expires (or expired).
    pub expires_at: UtcDateTime,
}

impl Session {
    /// Create a new session with a random UUID and expiry set to now +
    /// `IDLE_TIMEOUT`.
    pub fn new(now: UtcDateTime) -> Self {
        Self {
            id: Uuid::new_v4(),
            issued_at: now,
            expires_at: now + IDLE_TIMEOUT,
        }
    }

    fn expired(&self, now: UtcDateTime) -> bool {
        self.expires_at < now
    }
}

/// An in-memory store for managing sessions
#[derive(Actor)]
#[actor(name = "SessionStore")]
pub struct SessionStore {
    sessions: HashMap<SessionId, Session>,
}

impl SessionStore {
    /// Create an empty session store
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
        }
    }
}

/// Insert a new session or overwrite an existing one.
pub(super) struct Set {
    pub session: Session,
}

impl Message<Set> for SessionStore {
    type Reply = ();

    async fn handle(&mut self, msg: Set, _ctx: &mut Context<Self, Self::Reply>) -> Self::Reply {
        self.sessions.insert(msg.session.id, msg.session);
    }
}

/// Extend the idle timeout for a session. Returns `None` if the session is
/// missing or expired — callers should treat `None` as unauthenticated.
pub(super) struct Extend {
    pub id: SessionId,
    /// The current time, for checking expiry and computing the new deadline.
    pub now: UtcDateTime,
}

impl Message<Extend> for SessionStore {
    type Reply = Option<Session>;

    async fn handle(&mut self, msg: Extend, _ctx: &mut Context<Self, Self::Reply>) -> Self::Reply {
        let session = self.sessions.get(&msg.id).and_then(|session| {
            if session.expired(msg.now) {
                return None;
            }

            let new_expiry = msg.now + IDLE_TIMEOUT;
            let max_expiry = session.issued_at + MAX_SESSION_AGE;

            let expires_at = if new_expiry < max_expiry {
                new_expiry
            } else {
                max_expiry
            };

            let updated = Session {
                expires_at,
                ..session.to_owned()
            };

            Some(updated)
        });

        match session {
            Some(session) => {
                self.sessions.insert(msg.id, session.clone());
                Some(session)
            }
            None => {
                self.sessions.remove(&msg.id);
                None
            }
        }
    }
}

/// Remove a session from the store.
pub(super) struct Delete {
    pub id: SessionId,
}

impl Message<Delete> for SessionStore {
    type Reply = ();

    async fn handle(&mut self, msg: Delete, _ctx: &mut Context<Self, Self::Reply>) -> Self::Reply {
        self.sessions.remove_entry(&msg.id);
    }
}

/// Clear all expired sessions from the store. When `now` is `None` the
/// handler uses the current system time, which is the production path driven
/// by the scheduler. Pass `Some(now)` in tests to simulate time.
#[derive(Clone)]
pub struct ClearExpiredSessions {
    pub now: Option<UtcDateTime>,
}

impl ClearExpiredSessions {
    fn resolve_now(&self) -> UtcDateTime {
        self.now.unwrap_or_else(UtcDateTime::now)
    }
}

impl Message<ClearExpiredSessions> for SessionStore {
    type Reply = ();

    async fn handle(
        &mut self,
        msg: ClearExpiredSessions,
        _ctx: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        let now = msg.resolve_now();

        self.sessions.retain(|_id, session| !session.expired(now));
    }
}
/// Start the session store with a scheduler that clears expired sessions on a schedule.
pub async fn start_session_actor() -> Result<
    (ActorRef<SessionStore>, ActorRef<Scheduler>),
    SendError<SetInterval<SessionStore, ClearExpiredSessions>>,
> {
    let session_actor = SessionStore::spawn(SessionStore::new());
    let scheduler = Scheduler::spawn(Scheduler::new());

    scheduler
        .tell(SetInterval::new(
            session_actor.downgrade(),
            CLEAR_EXPIRED_SESSION_INTERVAL,
            ClearExpiredSessions { now: None },
        ))
        .await?;

    Ok((session_actor, scheduler))
}

#[cfg(test)]
mod tests {
    use kameo::actor::Spawn;
    use time::macros::utc_datetime;

    use super::*;

    #[tokio::test]
    async fn session_new_computes_expiry_correctly() {
        // Given a known point in time
        let now = utc_datetime!(2025-06-15 12:00:00);

        // When a session is created at that time
        let session = Session::new(now);

        // Then it has a non-nil UUID, correct timestamps, and correct expiry
        assert!(!session.id.is_nil());
        assert_eq!(session.issued_at, now);
        assert_eq!(session.expires_at, now + IDLE_TIMEOUT);
    }

    #[tokio::test]
    async fn extend_returns_none_for_expired_session() {
        // Given a session that was created 16 minutes ago (beyond IDLE_TIMEOUT)
        let now = utc_datetime!(2025-06-15 12:00:00);
        let session = Session::new(now);
        let store = SessionStore::spawn(SessionStore::new());
        store
            .tell(Set {
                session: session.clone(),
            })
            .await
            .unwrap();

        // When extending the session 1 second after it expired
        let after_expiry = session.expires_at + Duration::seconds(1);
        let result = store
            .ask(Extend {
                id: session.id,
                now: after_expiry,
            })
            .await
            .unwrap();

        // Then the result is None (session is expired)
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn extend_respects_max_session_age() {
        // Given a session near its absolute max age
        let base = utc_datetime!(2025-06-15 12:00:00);
        let session_expiry = base + MAX_SESSION_AGE - Duration::minutes(1);
        let session = Session {
            id: Uuid::new_v4(),
            issued_at: base,
            expires_at: session_expiry,
        };
        let store = SessionStore::spawn(SessionStore::new());
        store
            .tell(Set {
                session: session.clone(),
            })
            .await
            .unwrap();

        // When extending just before the absolute max age and before the session expiry
        let near_max_age = session_expiry - Duration::minutes(1);
        let result = store
            .ask(Extend {
                id: session.id,
                now: near_max_age,
            })
            .await
            .unwrap();

        // Then the session is valid but new expiry is clamped to the max age
        let extended = result.expect("session should still be valid");
        assert_eq!(extended.expires_at, base + MAX_SESSION_AGE);
    }

    #[tokio::test]
    async fn delete_removes_session() {
        // Given a valid session in the store
        let now = utc_datetime!(2025-06-15 12:00:00);
        let session = Session::new(now);
        let store = SessionStore::spawn(SessionStore::new());
        store
            .tell(Set {
                session: session.clone(),
            })
            .await
            .unwrap();

        // When the session is deleted
        store.tell(Delete { id: session.id }).await.unwrap();

        // Then extending it returns None
        let result = store
            .ask(Extend {
                id: session.id,
                now: now + Duration::minutes(1),
            })
            .await
            .unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn clear_expired_sessions_removes_only_expired() {
        // Given one expired and one valid session in the store
        let base = utc_datetime!(2025-06-15 12:00:00);
        let expired = Session::new(base);
        let valid = Session::new(base + Duration::minutes(10));
        let store = SessionStore::spawn(SessionStore::new());
        store
            .tell(Set {
                session: expired.clone(),
            })
            .await
            .unwrap();
        store
            .tell(Set {
                session: valid.clone(),
            })
            .await
            .unwrap();

        // When expired sessions are cleared at a time after the expired one's deadline
        let clear_time = expired.expires_at + Duration::minutes(1);
        store
            .tell(ClearExpiredSessions {
                now: Some(clear_time),
            })
            .await
            .unwrap();

        // Then the expired session is gone but the valid one remains
        let result = store
            .ask(Extend {
                id: expired.id,
                now: clear_time,
            })
            .await
            .unwrap();
        assert!(result.is_none(), "expired session should have been removed");

        let result = store
            .ask(Extend {
                id: valid.id,
                now: clear_time,
            })
            .await
            .unwrap();
        assert!(result.is_some(), "valid session should still be present");
    }
}
