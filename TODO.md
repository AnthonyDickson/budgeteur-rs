# To Do

## Stage One: Budgeting

- Add search/filtering to transactions page
- Add account info to transactions
  - Set during import
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
- Update values to use accounting formatting
  - Zero filled up to two decimal places for floats
  - Parantheses instead of minus symbol for negative values
  - Align digits and decimal point
- Use HTML5 elements where possible: https://dev.to/maxprilutskiy/html5-elements-you-didnt-know-you-need-gan
- Update unit tests to parse HTML document tree for checking for the existence
  of nodes and attributes.
- Refactor common testing functions into a separate module.
- Ensure page layout fits on smartphone screen
  - Transactions table
  - Accounts table
- Clean up/simplify HTML structure (e.g., remove redundant div wrappers), use semantic elements where possible
- Review UI design
  - [ ] Button states, ensure there is visual feedback for both hover and click (active) states
  - [ ] Rounded edge radii consistency---currently buttons use `rounded` but container uses `rounded-lg`
  - [ ] Autofocus on registration form
- Add support for Wise CSV exports
  - Complicated by multiple currencies

## Stage Two: Tracking Net Worth

TBC!

## Wishlist/Backlog

- Config server from TOML file.
  - Pagination items per page
