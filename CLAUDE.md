# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

**Setup:** `cargo run --bin create_test_db -- --output-path test.db` - Create the test database

**Development:**

- `bacon` - Development server (tailwindcss + server)
- `bacon test` - Test with file watching
- `bacon clippy` - Lint with file watching
- `cargo test` - Run tests
- `cargo clippy -- -D warnings` - Lint (warnings as errors)
- `cargo fmt` - Format code (run after verifying code works)
- `cargo run -- --db-path test.db` - Run the dev server

**Maintenance:**

- `sed -i 's/old/new/g' src/*.rs` - Batch string replacement
- `cargo run --bin reset_password -- --db-path test.db` - Reset password

## Architecture

**Stack:** Rust/Axum + SQLite + HTMX + Askama + TailwindCSS

### Key Patterns

**Direct Database Access**: Uses `Arc<Mutex<Connection>>` rather than abstract store patterns to modularize the codebase so that changes in one feature are isolated and do not affect other features.

**State Management**: Route handlers extract specific state slices using `FromRef` implementations rather than accessing full `AppState`.

**Route Organization**: API endpoints are mixed with view routes in the same handlers (not separated). Feature modules contain both database operations and route handlers.

**Database Conventions:**

- Functions take `connection: &Connection` as last parameter
- Use `AppState::new()` for initialization (handles DB setup automatically)
- Tests use in-memory databases with `get_test_connection()` helper
- HTML parsing tests should use document tree parsing rather than string matching

## Recent Changes

- **Transaction-Tag Refactoring (2025-09)**: Extracted transaction-tag relationship code from `tag.rs` into `transaction_tag.rs` to eliminate tight coupling
- **Tag Editing Feature (2025-08)**: Added full CRUD operations for tags including edit page at `/tags/:tag_id/edit` with PUT endpoint at `/api/tags/:tag_id`
- **Categories Renamed to Tags (2025-08)**: Renamed "categories" to "tags" throughout the application
- **Tag System Enhancement (2025-08)**: Added dedicated tags listing page at `/tags`
- **Tag-Transaction Separation (2025-08)**: Moved tag-related code from `transaction.rs` to dedicated `tag.rs` module with many-to-many mapping between tags and transactions
- **Store Pattern Removal (2025-08)**: Removed `TransactionStore` abstractions in favor of direct database function calls to modularize the codebase so that changes in one feature are isolated and do not affect other features.
- **Folder Structure Flattening (2025-08)**: `src/routes/mod.rs` → `src/routing.rs`, `src/auth/*` → `src/auth_*.rs`
- **Feature Module Organization (2025-08)**: All models moved to `src/` as self-contained modules
- **Code Quality Improvements (2025-08)**: All clippy warnings fixed, compiles with `-D warnings`
