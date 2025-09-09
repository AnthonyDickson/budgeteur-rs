# To Do

- Add CI/CD job on merge into main that checks that the Docker image can be built successfully.
- Add screenshots of app in README.md
- Add thousands separator to monetary amounts by implementing custom currency filter for Askama
- Add thousands separator to timing durations (e.g., 1,234ms instead of 1234ms) for better readability
- Organise the import, import_result and csv into a new module, `import` and only expose what's necessary. The `mod.rs` file should be minimal and just contain re-exports
- Change log in and registration pages to just ask for password
- Dashboard: create pie chart that breaks down spending by category 
- Dashboard: create line chart that charts monthly net income and balance (reconstruct balance from current balance and net income values)
- Port alerts system to other pages (other than rules page) for handling error messages
  - Use alerts for confirming deletion of items from tags and rules pages (and others when they get full CRUD).
  - Alert for dashboard if excluded tag ops fail.
  - Extend to offer undo capabilities on delete/edit?
  - Review how HTML code is shared, `{{ foo|safe }}` vs `{% include foo.html %}` vs `{% call my_macro(...) %}`
- Split up `src/transaction.rs` into module `src/transaction/*.rs`
- Ensure that simple and full csv imports from Kiwibank do not create duplicate
  transactions.
- Create unique aliases of `i64` for each of the domain models, e.g., `Tag` -> `pub type TagID = i64;`.
- Add edit and delete functionality for transactions.
- Add full CRUD functionality for balances
- Group transactions by week, month, year.
  - Add ISO week number to transaction
  - Add year number
- Either inline HTML files that just contain CSS classes or find a better way of reusing styles
- Config pagination (and other config) from toml file.
- Add command to reset user email
- Update unit tests to parse HTML document tree for checking for the existence
  of nodes and attributes.
- Reorganise tailwindcss code to use partials and/or custom styles instead of
  HTML templates.
  Refer to [v3.tailwindcss.com/docs/reusing-styles](https://v3.tailwindcss.com/docs/reusing-styles) and [v3.tailwindcss.com/docs/adding-custom-styles](https://v3.tailwindcss.com/docs/adding-custom-styles).
- Fix broken [icon](./static/seal.png).
- Refactor common testing functions into a separate module.
