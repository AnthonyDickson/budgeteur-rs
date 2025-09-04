# To Do

- Move non-shared code out of src/shared_templates.rs into the appropriate modules.
- Add rules-based auto-tagger (new branch, merge back w/ PR):
  - Add a user-configurable rules-based (text patterns) auto-tagging feature
  - Use rules-based tagger for automatically tagging imports
  - Add UI for manually trigger tagger on either all transactions or all untagged transactions
- Use rules-based tagger to mark internal transfers
- Add thousands separator to monetary amounts by implementing custom currency filter for Askama
- Align dashboard elements nicely
- Split up `src/transaction.rs` into module `src/transaction/*.rs`
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
- Reorganise tailwindcss code to use partials and/or custom styles instead of
  HTML templates.
  Refer to [v3.tailwindcss.com/docs/reusing-styles](https://v3.tailwindcss.com/docs/reusing-styles) and [v3.tailwindcss.com/docs/adding-custom-styles](https://v3.tailwindcss.com/docs/adding-custom-styles).
- Fix broken [icon](./static/seal.png).
- Refactor common testing functions into a separate module.
