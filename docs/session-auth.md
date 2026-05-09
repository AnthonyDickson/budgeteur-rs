# Session-Based Authentication

Server-side sessions via the Kameo `SessionStore` actor. The cookie carries an
opaque session ID (UUID v4); the server validates and manages the session
lifecycle.

## Auth Lifecycle

```
  Log In
  ├─ Verify password
  ├─ Session::new() → session_id (UUID v4)
  ├─ SessionStore::Set { id, issued_at, expires_at }
  ├─ Set cookie: auth_token = { session_id }
  └─ Redirect to dashboard

  Authenticated Request
  ├─ Extract session_id from cookie
  ├─ SessionStore::Extend { session_id }              ← verify + bump idle timer
  │  ├─ Some(session) → continue
  │  └─ None → redirect to /login                     ← expired or missing
  └─ Response (pass-through, no cookie manipulation)

  Log Out
  ├─ Extract session_id from cookie
  ├─ SessionStore::Delete { session_id }
  ├─ Invalidate cookie
  └─ Redirect to /login
```

## Session Lifecycle

| Event                 | Behavior                                                                      |
| --------------------- | ----------------------------------------------------------------------------- |
| Created (login)       | `issued_at = now`, `expires_at = now + IDLE_TIMEOUT` (15 min)                 |
| Extend (each request) | `expires_at = min(now + IDLE_TIMEOUT, issued_at + MAX_SESSION_AGE)` (max 24h) |
| Scheduler (hourly)    | Removes all sessions where `expires_at < now`                                 |

Cookie expires at `MAX_SESSION_AGE` (24h). The server manages session expiry
independently; the cookie just needs to outlive the session.

## Key Files

| File                     | Role                                                                                                           |
| ------------------------ | -------------------------------------------------------------------------------------------------------------- |
| `src/auth/session.rs`    | `Session` struct, `SessionStore` actor, messages (`Set`, `Extend`, `Delete`, `ClearExpiredSessions`) |
| `src/auth/token.rs`      | `Token { session_id: Uuid }` — serialized into the auth cookie                                                 |
| `src/auth/cookie.rs`     | `set_auth_cookie()`, `invalidate_auth_cookie()`, `get_token_from_cookies()`                                    |
| `src/auth/middleware.rs` | `auth_guard` / `auth_guard_hx` — extracts session ID, calls `Extend`, redirects on `None`                      |
| `src/auth/log_in.rs`     | `post_log_in` — verifies password, creates session, sets cookie                                                |
| `src/auth/log_out.rs`    | `get_log_out` — deletes session from actor, invalidates cookie                                                 |

## Design Decisions

1. **Session ID format** — UUID v4 (`uuid` crate). Strong randomness, minimal
   overhead.

2. **No `user_id` in sessions** — this is a single-user application. The
   `Session` struct carries only `id`, `issued_at`, and `expires_at`.

3. **No request extension** — `UserID` is not inserted into request extensions
   because no handler reads it. If a handler needs the user ID in future, it
   can use `UserID::new(1)` directly.

4. **Cookie duration** — matches `MAX_SESSION_AGE` (24h). No middleware
   cookie-extension logic. The cookie is set once at login; the browser sends it
   on every request until it expires.

5. **"Remember me"** — dropped. Single session length: `IDLE_TIMEOUT` 15 min,
   `MAX_SESSION_AGE` 24h.

6. **Atomic verify + extend** — the middleware calls `Extend` as a single
   message rather than `Verify` then `Extend`. This avoids a TOCTOU race where
   the session could expire between the two calls.

7. **Expired session cleanup** — the middleware does not clear cookies on
   `None` (expired/missing session). The login handler sets a fresh cookie on
   successful login, overwriting any stale one. The `ClearExpiredSessions`
   scheduler removes dead sessions from the actor every hour.

8. **Multiple concurrent sessions** — supported by the actor (map-based, no
   uniqueness constraint). Moot for a single-user app. If concurrent access
   becomes relevant, a session-listing endpoint or "log out everywhere" feature
   would be needed.
