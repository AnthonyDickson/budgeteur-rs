# To Do

## Stage One: Budgeting

- Implement expense cards, see [design spec](./docs/expenses-by-tag-design-spec.md) and
  [technical spec](./docs/expenses-by-tag-tech-spec.md)
- Currently shows up to 13 months (curr + curr of last year), remove first to avoid showing part of past month's transactions?
- Dashboard tables:
  - Make text in tables right aligned
  - Make it obvious that tables can be scrolled (always show scroll bar?)
  - Investigate flickering of sticky header column in monthly summaries table on iOS when scrolling vertically
  - Round values to whole numbers?
  - Reduce corner rounding from `rounded-lg` to `rounded`
- Bring registration form in line with other pages re how to handle errors, in particular mutex locks.
- Add support for Wise CSV exports
- Ensure all DB operations that are part of the import feature are atomic, i.e. all happen or none happen
- Organise code into modules based on features
  - dashboard
    - charts
    - routes
  - auth
    - log in
    - log out
    - cookies
    - middleware
    - password
    - user
  - tag
- Create unique aliases of `i64` for each of the domain models, e.g., `Tag` -> `pub type TagID = i64;`.
- Truncate long transaction descriptions and show full description in a tooltip
- On transactions page, group transactions by:
  - tag
  - day, week, fortnight, month, quarter, year
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
- Update dependencies
- Refactor common testing functions into a separate module.
- Ensure page layout fits on smartphone screen
- Review UI design
  - [ ] Button states, ensure there is visual feedback for both hover and click (active) states
  - [ ] Rounded edge radii consistency---currently buttons use `rounded` but container uses `rounded-lg`
  - [ ] Autofocus on registration form

## Stage Two: Tracking Net Worth

TBC!

## Wishlist/Backlog

- Config server from TOML file.
  - Pagination items per page
