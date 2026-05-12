# To Do

## Side Quest: Multi-Client Support (TUI)

### Bugs to fix

- [ ] **Medium:** Add `#[command(version)]` to TUI CLI so `--version` works
- [ ] **Medium:** Pre-compute DER-encoded signing key in `init()` instead of re-encoding via `to_pkcs8_der()` on every JWT
- [ ] **Medium:** `last_twelve_months` subtracts a flat 365 days — doesn't account for leap years
- [ ] **Medium:** Group TUI JWT re-exports separately from session auth types in `lib.rs`
- [ ] **Medium:** Health endpoint test only checks status code — add body shape assertions
- [ ] **Low:** `api_auth_guard` returns same `"invalid or expired token"` for both empty key store and bad JWT — distinguish for debuggability
- [ ] **Low:** `--tui-public-keys-path` defaults to relative `tui_public_keys.toml` — consider absolute path or document CWD dependency

### Views

- Dashboard view
- Transactions view
- CSV import
- Accounts view
- Tags view
- Auto-tagging rules view

## Stage One: Budgeting

- Add support for Wise CSV exports
  - Complicated by multiple currencies
- Add account info to transactions
  - Set during import
- Add search/filtering to transactions page
- Add page/widget on dashboard where you can check the impact of spending a specified amount:
  - Input for a positive amount, assume one-off
  - Net income chart
    - Actual net income for previous month
    - Current month should be mean net income over last 12 months minus the specified amount
    - Projections for the next ten months are the mean net income over the last 12 months
  - Balance Chart
    - Net balance for last month
    - Current month is previous month's balance plus mean net income over the last 12 months minus the specified amount
    - Projections for the next ten months are the above value plus the mean net income over the last 12 months

## Stage Two: Tracking Net Worth

TBC!

## Wishlist/Backlog

- Config server from TOML file.
  - Pagination items per page
