# To Do

## Stage One: Budgeting

- Migrate to Maud:
  - [ ] Rewrite askama templates to maud:
    - [x] templates/base.html
    - [ ] templates/components/nav_link.html
    - [x] templates/components/spinner.html
    - [ ] templates/macros/amount_display.html
    - [ ] templates/macros/transaction_summary.html
    - [ ] templates/partials/alert.html
    - [ ] templates/partials/dashboard_charts.html
    - [ ] templates/partials/edit_rule_form.html
    - [ ] templates/partials/edit_tag_form.html
    - [ ] templates/partials/error_page.html
    - [ ] templates/partials/import_form.html
    - [x] templates/partials/log_in/form.html
    - [ ] templates/partials/nav_bar.html
    - [ ] templates/partials/new_rule_form.html
    - [ ] templates/partials/new_tag_form.html
    - [ ] templates/partials/register/form.html
    - [ ] templates/partials/register/inputs/confirm_password.html
    - [ ] templates/partials/register/inputs/password.html
    - [ ] templates/partials/transaction_table_row.html
    - [ ] templates/partials/transaction_table_row_empty.html
    - [x] templates/styles/forms/input.html
    - [x] templates/styles/forms/label.html
    - [ ] templates/styles/text/plain.html
    - [ ] templates/views/account/accounts.html
    - [ ] templates/views/account/create.html
    - [ ] templates/views/account/edit.html
    - [ ] templates/views/dashboard.html
    - [ ] templates/views/dashboard_empty.html
    - [ ] templates/views/edit_rule.html
    - [ ] templates/views/edit_tag.html
    - [x] templates/views/forgot_password.html
    - [ ] templates/views/import.html
    - [ ] templates/views/internal_server_error_500.html
    - [x] templates/views/log_in.html
    - [x] templates/views/log_in_register_base.html
    - [ ] templates/views/new_rule.html
    - [ ] templates/views/new_tag.html
    - [ ] templates/views/not_found_404.html
    - [ ] templates/views/register.html
    - [ ] templates/views/rules.html
    - [ ] templates/views/tags.html
    - [ ] templates/views/transaction/create.html
    - [ ] templates/views/transaction/edit.html
    - [ ] templates/views/transaction/table.html
  - [ ] Remove Askama from dependencies
  - [ ] Delete templates folder
  - PR description:
    ```text
    This PR replaces Askama with Maud. Nesting, inheriting templates and sharing
    styles/components is awkward in Askama. I want something like Gleam's
    Lustre which allows me to write pure functions that generate the HTML in
    the host language (in this case, Rust). Maud also has the benefit that
    rendering is unfallible, removing the need for the `render` helper.
    ```

- Add table to dashboard that looks like:
  |            |    Jan |    Feb | ... |    Dec |   Total |
  | :--------- | -----: | -----: | --- | -----: | ------: |
  | Income     | $4,000 | $4,000 |     | $4,000 | $48,000 |
  | Expenses   | $3,000 | $4,500 |     | $3,000 | $45,000 |
  | Net Income | $1,000 |  -$500 |     | $1,000 |  $3,000 |
  | Balance    | $1,000 |   $500 |     | $1,000 |  $3,000 |

  For all rows, the total is the sum of all columns except for the balance which is simply the last value.
  Round all values to nearest dollar (banker's rounding?)
- Review how HTML code is shared, `{{ foo|safe }}` vs `{% include foo.html %}` vs `{% call my_macro(...) %}`
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

## Stage Two: Tracking Net Worth

TBC!

## Wishlist/Backlog

- Config server from TOML file.
  - Pagination items per page
