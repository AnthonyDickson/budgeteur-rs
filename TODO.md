# To Do

## Side Quest: Multi-Client Support (TUI)

- Reorganise project into workspaces?
  - server: Existing HTMX MPA + new JSON API layer
    - current `src/` folder
  - tui: New client
    - Add nix flake for easy distribution?
  - budgeteur (lib): Shared types etc.
  - How to manage deps such that building the TUI doesn't pull in server deps?
- TUI Skeleton
  - Ratatui
- Auth for TUI:
  - HMAC signature, shared secret
  - Signed JWT using assymmetric keys for passwordless access
    - For this setup, it would be nice to have a separate `config` and `data` folders
  - Config file for path, or just hardcode default and override via env var or CLI arg?

While adding JSON endpoints, try to extract common behaviour and separate response types (HTML, JSON) to keep shared
logic centralised.

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
