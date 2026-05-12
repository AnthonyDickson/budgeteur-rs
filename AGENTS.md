# Repository Guidelines

## Project Overview

Budgeteur is a personal finance web app built with Rust (Axum + Maud + HTMX) backed by SQLite. It serves HTML pages directly (MPA with HTMX for interactivity). The repo is a Cargo workspace with two crates: `server` (the web/mobile backend) and `tui` (a terminal client that connects to the server over HTTP from a different machine).

## Build, Test, and Development Commands

- `nix develop` — enter dev shell (Rust 1.95, bacon, Tailwind v4, `SECRET` env var).
- `cargo run -p budgeteur_rs --bin create_test_db -- --output-path test.db` — create a local test database (first time only).
- `bacon` — watch task runner. Press `r` to run the server, `t` to run tests, `c` for clippy-all, `f` for format, `d` for docs. See `bacon.toml` for all jobs.
- `cargo test -p budgeteur_rs` — run server test suite. `cargo test --workspace` for everything.
- `cargo test -q -p budgeteur_rs && cargo clippy -q --workspace --tests -- -D warnings && cargo fmt -- --check` — final quality check before committing.
- `cargo run -p budgeteur_tui -- --url http://localhost:8080` — run the TUI client that connects to the server from a different machine.
- `dprint fmt` — format Markdown and other non-Rust files.
- `./scripts/build_image.sh` — build Docker image, then `docker run --rm -p 8080:8080 -e SECRET=<YOUR-SECRET> -it ghcr.io/anthonydickson/budgeteur:dev`.
- Server binary flags: `--db-path`, `--timezone`, `--address`, `--port`, `--log-path`, `--tui-public-keys-path` (see `server/src/bin/server.rs`).

## Project Structure

```
Cargo.toml              # Workspace manifest (members: server, tui)
server/
  Cargo.toml            # Server crate (budgeteur_rs)
  src/
    bin/
      server.rs          # Entry point: CLI, tracing, session actor, router
      create_test_db.rs  # Creates test.db with all tables
      reset_password.rs  # CLI tool for password resets
    lib.rs               # Re-exports public API
    app_state.rs         # AppState: cookie_key, db_connection, session_actor, etc.
    routing.rs           # build_router() — route definitions, layered with middleware
    endpoints.rs         # Route path constants + format_endpoint() helper
    error.rs             # Error enum with IntoResponse (page errors) and into_alert_response()
    db.rs                # initialize() creates all tables in a single transaction
    html.rs              # Shared Maud components: base(), form styles, currency formatting
    alert.rs             # Alert enum (Success/Error) rendered as OOB HTMX swap
    logging.rs           # Request/response logging middleware, redacts passwords
    navigation.rs        # NavBar → bottom nav (mobile) + sidebar (desktop)
    timezone.rs          # get_local_offset() for timezone-aware date handling
    input.css            # Tailwind CSS input
    account/             # Account CRUD
    auth/                # Auth, sessions, JWT middleware for the TUI
    csv_import/          # CSV file upload + transaction import
    dashboard/           # Dashboard: cards, charts, tables, aggregation
    rule/                # Auto-tagging rules CRUD
    tag/                 # Tags CRUD, excluded tags
    transaction/         # Transactions CRUD, quick tagging, form rendering
    test_utils/          # Shared test helpers: form assertions, HTML parsing, HTTP helpers
tui/
  Cargo.toml            # TUI client crate — runs on a separate machine from the server
  src/
    main.rs             # CLI, --init key generation, Elm-style runtime loop
    app.rs              # Model, update, view, AuthenticatedClient, commands
    config.rs           # XDG config/data directory helpers
    runtime.rs          # Elm-style Cmd + Runtime for async effects
shared/
  Cargo.toml            # Shared library crate — types used by both server and TUI
  src/
    lib.rs              # Re-exports auth, dashboard, and routes modules
    auth.rs             # TuiClaims (JWT claims structure)
    dashboard.rs        # DashboardSummary (API response type)
    routes.rs           # Shared API route path constants
migrations/            # SQL migration scripts for schema upgrades
static/                # Built assets: main.css, HTMX JS, ECharts JS, favicons
docs/                  # Design and tech spec documents
```

## Control Flow

1. `server.rs` parses CLI args, opens SQLite, initializes DB tables, starts Kameo session/scheduler actors, builds router with `build_router(state)`.
2. `routing.rs` defines three route groups:
   - **Unprotected**: login, forgot password, health, coffee.
   - **Protected GET** (cookie sessions): Uses `auth_guard` — redirects to login on failure.
   - **Protected POST/PUT/DELETE** (cookie sessions): Uses `auth_guard_hx` — HTMX redirect on failure.
   - **JSON API** (bearer token): Uses `api_auth_guard` — returns `401` JSON on failure. Prefixed with `/api/v1/`.
3. Route handlers come from feature slices (see below). `routing.rs` maps paths to handler functions — there is no horizontal "api layer" between routes and slices.

## Vertical Slice Architecture

The project follows vertical slice architecture: each feature lives in its own directory. Slices own their HTTP handlers, DB queries, and response types — there is no separate "api layer".

### Per-slice file conventions

Slices start with:

- `mod.rs` — re-exports public handlers.
- `core.rs` — DB queries, domain models, shared business logic. No HTTP concerns.

For HTML endpoints (browser):

- `html.rs` — GET/POST/PUT/DELETE handlers returning HTML/Maud. Full pages and HTMX fragments.

For JSON endpoints (TUI client), when needed:

- `json.rs` — handlers returning JSON via `axum::Json`. Adds a sibling alongside the HTML handlers.

Both `html.rs` and `json.rs` import from `core.rs` and any shared pure-function modules (`aggregation.rs`, `form.rs`, etc.).
Response-type structs (`DashboardSummary` in `json.rs`, Maud `Markup` in `html.rs`) are format-specific and live in their respective file. Slices may have additional files for sub-concerns (`charts.rs`, `tables.rs`, `form.rs`, etc.).

Example layout for a slice with both HTML and JSON endpoints:

```
dashboard/
  mod.rs
  core.rs           # DB queries, build_dashboard_data, shared logic
  html.rs           # get_dashboard_page, update_excluded_tags (HTMX)
  json.rs           # get_dashboard_json (JSON for TUI)
  aggregation.rs     # Pure functions shared by both formats
  charts.rs
  tables.rs
```

### Profile slice (no endpoints, various helpers)

Module `auth/` is a helper module, not a feature slice. It contains authentication and authorisation related modules (cookies, JWT middleware, sessions) that cut across features.

### Route wiring

`routing.rs` imports handlers directly from slices — e.g. `dashboard::get_dashboard_page` for HTML, `dashboard::get_dashboard_json` for JSON.
The `/api/v1/` prefix is a routing concern only.

## Session Architecture

- `auth/session.rs` implements an in-memory `SessionStore` actor via [Kameo](https://github.com/tqwewe/kameo). Sessions have idle timeout (15 min) and max age (24 hours). A `Scheduler` actor periodically clears expired sessions.
- `AppState` holds `ActorRef<SessionStore>` and `ActorRef<Scheduler>`. The session actor is started in `server.rs` via `start_session_actor()` and passed into `AppState::new()`.
- `auth/middleware.rs` extracts the session ID from the cookie, calls `Extend` on the session actor (atomic verify + idle-bump), and redirects to login on `None`.
- The cookie carries `Token { session_id: Uuid }` — an opaque UUID v4, no user ID or expiry. The cookie expires at `MAX_SESSION_AGE` (24h); no middleware cookie-extension logic.
- See `docs/session-auth.md` for the full design.

## TUI Authentication (JWT)

The TUI authenticates via Ed25519-signed JWTs because it runs on a different machine and cannot use browser cookies.

- **Setup**: `budgeteur-tui init` generates a keypair. The private key stays on the TUI machine. The public key is added to the server's `tui_public_keys.toml`.
- **Runtime**: The TUI signs a short-lived JWT (1h) on each connection cycle and sends `Authorization: Bearer <jwt>`.
- **Middleware**: `auth/api_middleware.rs` validates the JWT against stored public keys. Returns `401` JSON on failure — no redirects.
- **Key store**: `auth/api_keys.rs` loads allowed public keys from TOML at startup.
- The `/api/v1/*` route group is protected by `api_auth_guard`.

## Dependencies

When adding a new dependency, use `cargo` to ensure the latest version is installed and then edit the `Cargo.toml` so that the dependency is pinned to the latest major version (for packages at version 1+) or latest minor version (for packages < 1.0.0).

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

- **JSON endpoints**: Return `Error` directly (it implements `IntoResponse` and the framework will serialize it as JSON via the `axum::Json` extractor). For more control, return `Result<Json<T>, Error>`.

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
- Handler tests call the handler function directly with `State(state)` + `Form(form)` and inspect the response.
- Integration-style tests use `axum-test::TestServer` with `Router::new()`.
- Test middleware layers independently from the handlers they protect.
- `src/test_utils/` provides shared helpers:
  - `form::` — `must_get_form()`, `assert_form_input()`, `assert_form_input_with_value()`, `assert_form_submit_button()`, `assert_hx_endpoint()`, `assert_form_error_message()` (all use `scraper` crate)
  - `html::` — `parse_html_document()`, `parse_html_fragment()`, `assert_valid_html()`
  - `http::` — `assert_status_ok()`, `assert_content_type()`, `get_header()`, `assert_hx_redirect()`
- Module-specific test utils (e.g., `transaction/test_utils.rs`) provide domain assertions like `assert_transaction_type_inputs()`.
- When adding form fields, add a regression test asserting the input exists with correct default/checked state.
- Structure tests around the Given-When-Then pattern, use comments to block out each test.

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
- Use `tracing` (not `log`) for all logging. Keep logging at call sites, not inside pure data modules.
- Date/time: use `time` crate (not `chrono`). Default to user's local timezone via `get_local_offset()`, not UTC.
- When replacing subsystems, delete unused modules/tests — never silence dead-code warnings.
- Follow the functional core, imperative shell pattern. Business logic lives in the functional core; non-deterministic
  behaviour (system time, random data, I/O) lives outside the core.
- Modules that are pure data (key stores, config parsing) should not contain tracing/logging calls — push those to the call sites.

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
