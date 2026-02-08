# Repository Guidelines

## Project Structure & Module Organization

- `src/` contains the Rust application code, split into feature modules (e.g., `account/`, `transaction/`, `tag/`, `rule/`, `dashboard/`) plus `bin/` entry points (`server`, `create_test_db`, `reset_password`).
- `static/` holds built web assets (for example, `static/main.css` from Tailwind).
- `assets/` stores documentation images.
- `migrations/` contains database migration scripts.
- `docs/` holds design and technical specs; `DOCS.md` is the developer guide.
- `scripts/` contains helper scripts such as `build_image.sh`.

## Build, Test, and Development Commands

- `nix develop` to enter the pinned dev environment (Rust toolchain, bacon, Tailwind, test env vars).
- `cargo run --bin create_test_db -- --output-path test.db` to create a local test database (first time).
- `bacon` to run the watch task runner; press `r` to run the server, `t` to run tests, `d` to build docs.
- `cargo test` to run the full test suite outside bacon.
- `./scripts/build_image.sh` to build the Docker image, then `docker run --rm -p 8080:8080 -e SECRET=<YOUR-SECRET> -it ghcr.io/anthonydickson/budgeteur:dev`.

## Coding Style & Naming Conventions

- Follow Rust standard style: `snake_case` for functions/variables, `PascalCase` for types.
- Prefer `Error`’s `IntoResponse` for page endpoints; insert errors into forms or use `AlertTemplate` for fragments.
- Use `bacon.toml` jobs for quality checks (`cargo check`, `cargo clippy`, `cargo doc`).

## Testing Guidelines

- Tests live alongside code in `src/` modules with `#[cfg(test)]` and `#[test]`/`#[tokio::test]`.
- Run via `bacon` (`t`) or `cargo test`. Database-related tests often rely on `test.db`.

## View Templates & Test Helpers

- Keep `html!` blocks presentation-focused by preparing values above the markup; for example, precompute a `TransactionFormDefaults` before rendering and pass it into a shared helper.
- When the same form appears in multiple pages, extract a shared renderer (e.g., transaction form fields live in `src/transaction/form.rs` and are reused by create/edit views).
- Prefer shared test helpers for repeated HTML assertions (e.g., `src/transaction/test_utils.rs` centralizes HTML parsing and form input checks).
- For new form fields, add a minimal regression test that asserts the input exists and the default selected/checked state (e.g., transaction `type_` radio inputs).

## Commit & Pull Request Guidelines

- Commit messages follow a conventional pattern like `feat: ...` or `refactor: ...` (often with a PR number, e.g., `(#99)`).
- PRs should include a clear description, link relevant issues if any, and add screenshots for UI changes.
- Ensure tests pass and update docs/specs when behavior or UI changes.

## Security & Configuration Tips

- For local development, use the `SECRET` environment variable (provided by `nix develop` or manually).
- The default server runs on HTTP; use a reverse proxy for HTTPS in real deployments.

## Date/Time Conventions

- Prefer explicit window presets (week/fortnight/month/quarter/half-year/year) for date-range navigation and include full four-digit years on both ends of range labels.
- Default “now” calculations to the user’s local timezone via `get_local_offset`, not UTC.
- For windowed navigation, use target date ranges as link text to make direction unambiguous.

## Dev Process

- When finished implementing or refactoring something, run `cargo test -q` and `cargo clippy --quiet`.
- When replacing a subsystem (e.g., pagination → windows), remove unused modules/tests rather than silencing dead-code warnings.
