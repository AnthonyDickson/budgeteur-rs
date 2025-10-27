# To Do

## Stage One: Budgeting

- For transactions page, send pagination params to edit page so it can redirect back the same page.
- Investigate why editing a transaction is slow on NAS (600ms-1,300ms per request)
- For rules page, check whether its using `hx-swap="delete"`, avoid a page reload.
- Add full CRUD functionality for balances
- Page for quickly tagging untagged transactions
- Prompt user to add transactions on dashboard page if the user has no transactions.
- Port alerts system to other pages (other than rules page) for handling error messages
  - Use alerts for confirming deletion of items from tags and rules pages (and others when they get full CRUD).
  - Alert for dashboard if excluded tag ops fail.
  - Extend to offer undo capabilities on delete/edit?
  - Review how HTML code is shared, `{{ foo|safe }}` vs `{% include foo.html %}` vs `{% call my_macro(...) %}`
- Log errors at source to make debugging easier
- Ensure all DB operations that are part of the import feature are atomic, i.e. all happen or none happen
- Organise code into modules based on features
  - dashboard
  - auth
    - log in
    - log out
    - cookies
    - middleware
    - password
    - user
  - tag
- Create unique aliases of `i64` for each of the domain models, e.g., `Tag` -> `pub type TagID = i64;`.
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
- Use macro for transactions table rows instead of nested template? Same for form inputs?
- Either inline HTML files that just contain CSS classes or find a better way of reusing styles
- Update unit tests to parse HTML document tree for checking for the existence
  of nodes and attributes.
- Reorganise tailwindcss code to use partials and/or custom styles instead of
  HTML templates.
  Refer to [v3.tailwindcss.com/docs/reusing-styles](https://v3.tailwindcss.com/docs/reusing-styles) and [v3.tailwindcss.com/docs/adding-custom-styles](https://v3.tailwindcss.com/docs/adding-custom-styles).
- Refactor common testing functions into a separate module.
- Ensure page layout fits on smartphone screen
- Consider merging "api" paths into root router
  - For example, `DELETE api/transactions/{transaction_id}` -> `DELETE transactions/{transaction_id}`
- Upgrade Rust and dependencies
- Upgrade to Tailwind CSS 4

## Stage Two: Tracking Net Worth

TBC!

## Wishlist/Backlog

- Config server from TOML file.
  - Pagination items per page
