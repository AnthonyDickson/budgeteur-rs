# To Do

## Side Quest: Multi-Client Support (TUI)

- Dashboard view
  - Rearrange to put expenses by tag and untagged transaction on a third row that fills the height
  - Set ignored tags similar to dashboard
    - Popup to set ignored tags or tags & rules page?
      - Argument for dashboard: the tag exclusion only applies to the dashboard currently
      - Argument for tags & ruls page: this is a setting related to tags
  - Add indication of chart period somewhere (1Y/last 12 months)
  - Add row:
    - Add chart for monthly expenses
      - Check if ratatui supports stacked bar charts
        - It does, just pass in a `Vec<Dataset>`, order matters (last is rendered last/on top)
    - Add table for monthly breakdown of income, expenses, net income
      - Monthly summary table can span two cols to reduce scrolling
  - Quick tag from untagged transaction widget
  - Handle case where the user has no data
    - expenses by tag view: prompt user to add transactions and tag them
  - Once accounts are set up to differentiate between liquid and fixed assets, and short term and long term liabilities,
    update the savings stats to only count liquid assets and short term liabilities towards the savings. The net worth
    calculation can continue to aggregate across all accounts.
- Transactions view
- CSV import
  - Custom file picker
  - Display as popup/dialog
- Accounts view
- Tags view
- Auto-tagging rules view
- Enforce a minimum screen size

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
