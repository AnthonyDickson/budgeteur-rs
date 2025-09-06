# To Do

- Add rules-based auto-tagger (new branch, merge back w/ PR):
  - [ ] Use rules-based tagger for automatically tagging imports
- Add settings page to configure which tags to exclude in dashboard (internal transfer tag created by user)
- Ensure a tag can be applied to a transaction only once
- Add thousands separator to monetary amounts by implementing custom currency filter for Askama
- Align dashboard elements nicely
- Change log in and registration pages to just ask for password
- Port alerts system to other pages (other than rules page) for handling error messages
- Split up `src/transaction.rs` into module `src/transaction/*.rs`
- Ensure that simple and full csv imports from Kiwibank do not create duplicate
  transactions.
- Create unique aliases of `i64` for each of the domain models, e.g., `Tag` -> `pub type TagID = i64;`.
- Align transactions in /transactions to the right.
- Group transactions by week, month, year.
  - Add ISO week number to transaction
  - Add year number
- Config pagination (and other config) from toml file.
- Add command to reset user email
- Update unit tests to parse HTML document tree for checking for the existence
  of nodes and attributes.
- Reorganise tailwindcss code to use partials and/or custom styles instead of
  HTML templates.
  Refer to [v3.tailwindcss.com/docs/reusing-styles](https://v3.tailwindcss.com/docs/reusing-styles) and [v3.tailwindcss.com/docs/adding-custom-styles](https://v3.tailwindcss.com/docs/adding-custom-styles).
- Fix broken [icon](./static/seal.png).
- Refactor common testing functions into a separate module.
