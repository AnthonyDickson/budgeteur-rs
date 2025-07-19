# To Do

- Update dashboard
  - Add trailing 12 month net income
  - Add net balance
- Add Buttons to navigate to first/last pages
  - Place below page numbers?
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
