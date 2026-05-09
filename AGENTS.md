# Repository Guidelines

## Project Overview

Budgeteur is a personal finance web app built with Rust (Axum + Maud + HTMX) backed by SQLite. It serves HTML pages directly (MPA with HTMX for interactivity). Single binary `server` handles everything; no JavaScript build step.

## Build, Test, and Development Commands

- `nix develop` — enter dev shell (Rust 1.95, bacon, Tailwind v4, `SECRET` env var).
- `cargo run --bin create_test_db -- --output-path test.db` — create a local test database (first time only).
- `bacon` — watch task runner. Press `r` to run the server, `t` to run tests, `c` for clippy-all, `f` for format, `d` for docs. See `bacon.toml` for all jobs.
- `cargo test` — run full test suite standalone.
- `cargo test -q && cargo clippy -q && cargo fmt` — final quality check before committing.
- `dprint fmt` — format Markdown and other non-Rust files.
- `./scripts/build_image.sh` — build Docker image, then `docker run --rm -p 8080:8080 -e SECRET=<YOUR-SECRET> -it ghcr.io/anthonydickson/budgeteur:dev`.
- Server binary flags: `--db-path`, `--timezone`, `--address`, `--port`, `--log-path` (see `src/bin/server.rs:27-49`).

## Project Structure & Architecture

```
src/
  bin/
    server.rs          # Entry point: parses CLI, sets up tracing, session actor, router
    create_test_db.rs  # Creates test.db with all tables
    reset_password.rs  # CLI tool for password resets
  lib.rs               # Re-exports AppState, Error, build_router, initialize_db, etc.
  app_state.rs         # AppState: cookie_key, db_connection (Arc<Mutex<Connection>>), session_actor, scheduler
  routing.rs           # build_router(): unprotected routes, protected GET routes (auth_guard), protected mutating routes (auth_guard_hx)
  endpoints.rs         # All route path constants + format_endpoint() helper
  error.rs             # App-level Error enum with IntoResponse (page errors) and into_alert_response() (fragment errors)
  db.rs                # initialize() creates all tables in a single transaction
  html.rs              # Shared Maud components: base(), error_view(), form styles, buttons, currency formatting
  alert.rs             # Alert enum (Success/Error) rendered as OOB HTMX swap into #alert-container
  logging.rs           # Request/response logging middleware that redacts passwords
  navigation.rs        # NavBar struct → bottom nav (mobile) + sidebar (desktop)
  timezone.rs          # get_local_offset() for timezone-aware date handling
  input.css            # Tailwind CSS input
  account/             # CRUD pages + endpoints
  auth/                # Login, logout, forgot password, middleware, cookie handling, sessions
  csv_import/          # CSV file upload + transaction import
  dashboard/           # Dashboard views: cards, charts, tables, aggregation
  rule/                # CRUD for auto-tagging rules
  tag/                 # CRUD for tags, excluded tags, preferences
  transaction/         # Core transaction logic, CRUD pages/endpoints, quick tagging, form rendering
    quick_tagging/     # Sub-module: HTMX-driven quick tagging workflow for untagged imports
  test_utils/          # Shared test helpers: form assertions, HTML parsing, HTTP response helpers
migrations/            # SQL migration scripts for schema upgrades
static/                # Built assets: main.css (Tailwind output), HTMX JS, ECharts JS, favicons
docs/                  # Design and tech spec documents
```

### Control Flow

1. `server.rs` parses CLI args, opens SQLite, initializes DB tables, starts Kameo session/scheduler actors, builds router with `build_router(state)`.
2. `routing.rs` splits routes into unprotected (login, forgot password, coffee) and protected (everything else). Protected GET routes use `auth_guard` (redirect on failure). Protected POST/PUT/DELETE routes use `auth_guard_hx` (HxRedirect on failure).
3. Each feature module follows the same pattern: `mod.rs` re-exports, `core.rs` for models/DB queries, `*_page.rs` for GET handlers + HTML rendering, `*_endpoint.rs` for POST/PUT/DELETE handlers.

### Session Architecture

- `auth/session.rs` implements an in-memory `SessionStore` actor via [Kameo](https://github.com/tqwewe/kameo). Sessions have idle timeout (15 min) and max age (24 hours). A `Scheduler` actor periodically clears expired sessions.
- `AppState` holds `ActorRef<SessionStore>` and `ActorRef<Scheduler>`. The session actor is started in `server.rs` via `start_session_actor()` and passed into `AppState::new()`.
- `auth/middleware.rs` extracts the session ID from the cookie, calls `Extend` on the session actor (atomic verify + idle-bump), and redirects to login on `None`.
- The cookie carries `Token { session_id: Uuid }` — an opaque UUID v4, no user ID or expiry. The cookie expires at `MAX_SESSION_AGE` (24h); no middleware cookie-extension logic.
- See `docs/session-auth.md` for the full design.

## State Extraction Pattern

Handlers extract state via `FromRef` implementations on per-endpoint state structs:

```rust
#[derive(Debug, Clone)]
pub struct CreateAccountState {
    pub db_connection: Arc<Mutex<Connection>>,
}

impl FromRef<AppState> for CreateAccountState {
    fn from_ref(state: &AppState) -> Self {
        Self { db_connection: state.db_connection.clone() }
    }
}
```

Never extract `Arc<Mutex<Connection>>` directly from `AppState` — create a dedicated state struct. When acquiring the lock, never `unwrap()`; log the error and return `Error::DatabaseLockError`.

## Error Handling

Two response paths depending on context:

- **Page endpoints** (full page responses): Implement `IntoResponse` for `Error`. Most errors map to `InternalServerError` or `NotFoundError` templates. The `?` operator works directly in handlers returning `impl IntoResponse`.

- **Fragment endpoints** (HTMX responses): Call `error.into_alert_response()` which returns `(StatusCode, Alert::into_html())`. The Alert renders as an OOB swap into `#alert-container` (defined in `base()` template).

- Always log errors at the callsite: `tracing::error!("Could not create account: {error}");`

- DB locking pattern (always use this):
  ```rust
  let connection = match state.db_connection.lock() {
      Ok(conn) => conn,
      Err(error) => {
          tracing::error!("could not acquire database lock: {error}");
          return Error::DatabaseLockError.into_alert_response();
      }
  };
  ```

- Success responses for mutating endpoints: return `(HxRedirect(target), StatusCode::SEE_OTHER).into_response()`.

## Testing Patterns

- Tests live alongside code with `#[cfg(test)]` and `#[test]`/`#[tokio::test]`.
- Database tests use `Connection::open_in_memory()` + `initialize(&conn)` for a fresh SQLite instance per test.
- Standard pattern for endpoint tests:
  ```rust
  fn get_test_connection() -> Connection {
      let conn = Connection::open_in_memory().unwrap();
      initialize(&conn).unwrap();
      conn
  }
  ```
- Handler tests call the handler function directly with `State(state)` + `Form(form)` and inspect the response.
- Integration-style tests use `axum-test::TestServer` with `Router::new()`.
- `src/test_utils/` provides shared helpers:
  - `form::` — `must_get_form()`, `assert_form_input()`, `assert_form_input_with_value()`, `assert_form_submit_button()`, `assert_hx_endpoint()`, `assert_form_error_message()` (all use `scraper` crate)
  - `html::` — `parse_html_document()`, `parse_html_fragment()`, `assert_valid_html()`
  - `http::` — `assert_status_ok()`, `assert_content_type()`, `get_header()`, `assert_hx_redirect()`
- Module-specific test utils (e.g., `transaction/test_utils.rs`) provide domain assertions like `assert_transaction_type_inputs()`.
- When adding form fields, add a regression test asserting the input exists with correct default/checked state.

## Templating (Maud + HTMX)

- Use `maud::html!` macro for all HTML rendering. Keep `html!` blocks presentation-focused: precompute values above the template.
- All pages use `html::base(title, &head_elements, &content)` which provides DOCTYPE, HTMX scripts, favicon, alert container, and body styles.
- `HEAD` elements (scripts, styles) are passed as `&[HeadElement]` — use `HeadElement::ScriptLink()`, `HeadElement::ScriptSource()`, `HeadElement::Style()`.
- CSS classes are defined as `&str` constants in `html.rs` (e.g., `FORM_TEXT_INPUT_STYLE`, `TABLE_ROW_STYLE`). Use these constants, don't inline Tailwind classes.
- HTMX attributes: `hx-post`, `hx-put`, `hx-delete`, `hx-target`, `hx-swap`, `hx-confirm`, `hx-target-error="#alert-container"` (for fragment-alert error responses).
- Reusable form components should be extracted into shared renderers (e.g., `transaction/form.rs`).
- Currency formatting: use `html::format_currency()` (2 decimal places) or `html::format_currency_rounded()` (whole dollars). The `numfmt` crate loses trailing zeros — `format_currency()` manually appends them.

## Coding Style & Conventions

- Rust 2024 edition. `snake_case` for functions/variables, `PascalCase` for types.
- Prefer imports at top of file; avoid inline qualified imports.
- Avoid abstraction unless it meaningfully reduces code volume or improves readability/reasoning.
- No comments that restate code; only document non-obvious rationale, invariants, or gotchas.
- Use `tracing` (not `log`) for all logging.
- Date/time: use `time` crate (not `chrono`). Default to user's local timezone via `get_local_offset()`, not UTC.
- When replacing subsystems, delete unused modules/tests — never silence dead-code warnings.
- Try to follow the functional core, imperative shell pattern. Business logic should mostly live in the functional core
  and sources of non-deterministic behaviour such as reading the system time, generating random data, I/O should live
  outside the core.

## Commit & PR Guidelines

- Commit messages: `feat: ...`, `fix: ...`, `refactor: ...`, `chore: ...` — often with PR number suffix e.g. `(#99)`.
- PRs: clear description, link issues, screenshots for UI changes.
- Ensure tests pass and update docs/specs.

## Important Gotchas

- **Safari localhost**: Secure cookies on localhost don't work in Safari. Use Chrome/Firefox for local testing.
- **Cookie path**: Always set `.path("/")` on auth cookies to avoid browser omitting cookies for parent paths.
- **HTMX redirects vs regular redirects**: Protected GET routes use `Redirect` (303). Protected POST/PUT/DELETE routes must use `HxRedirect` for HTMX to follow the redirect. This is why `routing.rs` has two separate `protected_routes` blocks with different middleware.
- **numfmt trailing zeros**: `numfmt::Formatter::currency()` drops the last trailing zero (e.g., `$12.30` → `$12.3`). `format_currency()` manually appends `"0"` when needed.
- **SQLite locking**: DB connection is `Arc<Mutex<Connection>>`. Always handle lock errors, never `unwrap()`.
- **Rust edition 2024**: The project uses edition 2024 — some syntax differs from 2021 (e.g., `use` ordering, `unsafe` blocks).
- **Markdown formatting**: After editing any Markdown file, run `dprint fmt` to format it.
