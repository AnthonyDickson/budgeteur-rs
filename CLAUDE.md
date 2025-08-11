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

1. **Direct Database Access**: The codebase uses direct database connections via `Arc<Mutex<Connection>>` rather than abstract store patterns. All database operations use functions that take `rusqlite::Connection` as a parameter (typically as the last argument).

2. **State Management**: The `AppState` struct holds shared application state including database connections, cookie keys, and pagination config. Route handlers extract specific state slices using `FromRef` implementations.

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
- `src/state.rs` - Application state and dependency injection with SQLite initialization
- `src/routes/mod.rs` - Route definitions and middleware
- `src/transaction.rs` - Transaction database operations, queries, and route handlers
- `src/auth/` - Authentication middleware and cookie handling
- `src/models/` - Domain models (User, Transaction, etc.)
- `src/*.rs` - Individual feature modules (balances, category, user, etc.) containing database operations and business logic

**Testing Strategy:**
- Integration tests use in-memory SQLite databases
- Route testing with `axum-test` crate
- HTML parsing tests should use document tree parsing rather than string matching

**Database Function Conventions:**
When working with database operations, follow these patterns:
- Database functions typically take connection as the last parameter: `fn operation(params, connection: &Connection)`
- Use `Arc<Mutex<Connection>>` for shared database access in route handlers
- All tests use in-memory databases with the `get_test_connection()` helper function
- Import database functions directly rather than using qualified imports
- Create `AppState` with `AppState::new(connection, cookie_secret, pagination_config)` - it handles database initialization automatically

## Recent Changes

- **Store Pattern Removal (2025-08)**: The codebase previously used abstract
   `TransactionStore` and `SQLiteTransactionStore` patterns. These have been
   removed in favor of direct database function calls to modularise the codebase
   so that changes in one feature are isolated and do not affect other features.
   All functionality remains the same but with cleaner architecture.

- **Route Handler Consolidation (2025-08)**: Transaction route handlers have been
   merged into `src/transaction.rs` for better code organization and feature isolation.

- **Model Organization (2025-08)**: All remaining models have been moved to the
   `src/` directory as individual feature modules, improving code organization
   and making features more self-contained.
