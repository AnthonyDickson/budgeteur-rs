# To Do

- Rework AppState to replace stores with rusqlite Connection
  - Replace usage of `CategoryStore` and `SQLiteCategoryStore` with functions that take database connection
  - Code using stores can be replaced with functions that use the connection directly
  - Queries can be optimised to just what's needed
  - Functions using the connection can be mocked by using function pointers
  - Tests can define functions that are injected into the tested code
  - Tests should use in-memory database for integration tests and ensure all tests
    are initialised with the same schema
- Remove accessors from `User` struct in `src/models/user.rs`.
- Update dashboard
  - Add trailing 1 month summary (income, expenses, net income)
  - Add trailing 12 month summary (income, expenses, net income)
  - Add net balance from account balances
- Ensure that simple and full csv imports from Kiwibank do not create duplicate
  transactions.
- Align transactions in /transactions to the right.
- Group transactions by week, month, year.
  - Add ISO week number to transaction
  - Add year number
- Config pagination (and other config) from toml file.
- Add command to reset user email
- Update unit tests to parse HTML document tree for checking for the existence
  of nodes and attributes.
- Move route handler code for views to routes/views module.
- Reorganise tailwindcss code to use partials and/or custom styles instead of
  HTML templates.
  Refer to [v3.tailwindcss.com/docs/reusing-styles](https://v3.tailwindcss.com/docs/reusing-styles) and [v3.tailwindcss.com/docs/adding-custom-styles](https://v3.tailwindcss.com/docs/adding-custom-styles).
- Fix broken [icon](./static/seal.png).
- Refactor common testing functions into a separate module.
