# To Do

## Stage One: Budgeting

- Review whether displaying IDs (e.g., transactions view, tags view) is useful
- Move `import_transaction_list` to `csv_import/import_transactions.rs`
- Error alert if import fails
- Port alerts system to other pages (other than rules page) for handling error messages
  - Use alerts for confirming deletion of items from tags and rules pages (and others when they get full CRUD).
  - Alert for dashboard if excluded tag ops fail.
  - Extend to offer undo capabilities on delete/edit?
  - Review how HTML code is shared, `{{ foo|safe }}` vs `{% include foo.html %}` vs `{% call my_macro(...) %}`
- Split up `src/transaction.rs` into module `src/transaction/*.rs`
- Create unique aliases of `i64` for each of the domain models, e.g., `Tag` -> `pub type TagID = i64;`.
- Add edit and delete functionality for transactions.
- Add full CRUD functionality for balances
- Prompt user to add transactions on dashboard page if the user has no transactions.
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
- Upgrade Rust and dependencies
- Upgrade to Tailwind CSS 4

## Stage Two: Tracking Net Worth

TBC!

## Wishlist/Backlog

- Config server from TOML file.
  - Pagination items per page
