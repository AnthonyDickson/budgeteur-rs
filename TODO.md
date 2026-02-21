# To Do

## Stage One: Budgeting

- Remove obsolete `static/app.js`
- Rework nav
  - Navbar should contain links to pages for creating new transactions, accounts, tags, rules as dropdowns as well as
    links to transaction imports and quick tagging
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
