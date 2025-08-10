# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Development Commands

**Setup (first time only):**
```bash
cargo run --bin create_test_db -- --output-path test.db
```

**Development with Bacon:**
- `bacon` - Start development environment (runs tailwindcss + server)
- `bacon run` - Run tailwindcss and start server on port 3000
- `bacon test` - Run all tests with file watching
- `bacon clippy` - Run clippy lints
- `bacon doc-open` - Build and open documentation

**Manual Development:**
- `cargo run -- --db-path test.db` - Start server manually
- `cargo test` - Run tests
- `cargo clippy` - Lint code
- `tailwindcss -i templates/source.css -o static/main.css` - Compile CSS

**Database Management:**
- `cargo run --bin reset_password -- --db-path test.db` - Reset user password

## Architecture Overview

**Technology Stack:**
- **Backend**: Rust with Axum web framework
- **Database**: SQLite with direct rusqlite connections
- **Frontend**: Server-rendered HTML with HTMX for dynamic interactions
- **Templates**: Askama templating engine
- **Styling**: TailwindCSS

**Key Architectural Patterns:**

1. **Store Pattern Transition**: The codebase is actively refactoring away from abstract store traits to direct database connections. Prefer using `Arc<Mutex<Connection>>` directly rather than the `TransactionStore` trait for new code.

2. **State Management**: The `AppState<T>` struct holds shared application state including database connections, cookie keys, and pagination config. Route handlers extract specific state slices using `FromRef` implementations.

3. **Authentication**: Cookie-based authentication using private encrypted cookies. The `auth::middleware` module provides guards for protected routes.

4. **Error Handling**: Centralized error handling through the `Error` enum in `lib.rs` with automatic conversion from `rusqlite::Error` and HTTP response mapping.

5. **Route Organization**:
   - `routes/endpoints.rs` - URL constants
   - `routes/views/` - Page handlers that return HTML
   - API endpoints mixed with view routes in the same handlers
   - HTMX-specific headers handled in `routes/mod.rs`

**Database Schema Management:**
- Tables are created through `CreateTable` trait implementations
- Models use `MapRow` trait for SQL result mapping
- Direct SQL queries preferred over ORM abstractions

**Key Modules:**
- `src/state.rs` - Application state and dependency injection
- `src/routes/mod.rs` - Route definitions and middleware
- `src/stores/sqlite/` - SQLite-specific database implementations
- `src/auth/` - Authentication middleware and cookie handling
- `src/models/` - Domain models (User, Transaction, etc.)

**Testing Strategy:**
- Integration tests use in-memory SQLite databases
- Route testing with `axum-test` crate
- HTML parsing tests should use document tree parsing rather than string matching

**Current Refactoring Focus:**
The codebase is actively removing the store pattern abstraction layer in favor of direct database access. When working with database operations, prefer injecting `Arc<Mutex<Connection>>` directly rather than using store traits.