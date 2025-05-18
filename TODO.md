# To Do

- Update unit tests to parse HTML document tree for checking for the existence
  of nodes and attributes.
- Move route handler code for views to routes/views module.
- Reorganise tailwindcss code to use partials and/or custom styles instead of
  HTML templates.
  Refer to [v3.tailwindcss.com/docs/reusing-styles](https://v3.tailwindcss.com/docs/reusing-styles) and [v3.tailwindcss.com/docs/adding-custom-styles](https://v3.tailwindcss.com/docs/adding-custom-styles).
- Fix broken [icon](./static/seal.png).
- Refactor common testing functions into a separate module.
- Track account and credit card balances:
  - Add page to view balances.
  - Add to database:
    - CREATE: Table for bank/credit card accounts with:
      - Numeric ID: INTEGER
      - Account/card number: TEXT
    - CREATE: Table for balances with:
      - Account/card ID: Integer
      - Date: TEXT
      - Balance: REAL
  - Add route for reading csv files and populating transactions.
    - For both credit cards and bank accounts, extract the following fields:
      - Account number
    - For bank accounts, extract:
      - Balance
