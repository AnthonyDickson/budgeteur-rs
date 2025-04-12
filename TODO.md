# To Do

- Update unit tests to parse HTML document tree for checking for the existence
  of nodes and attributes.
- Move route handler code for views to routes/views module.
- Reorganise tailwindcss code to use partials and/or custom styles instead of
  HTML templates.
  Refer to [v3.tailwindcss.com/docs/reusing-styles](https://v3.tailwindcss.com/docs/reusing-styles) and [v3.tailwindcss.com/docs/adding-custom-styles](https://v3.tailwindcss.com/docs/adding-custom-styles).
- Fix broken [icon](./static/seal.png).
- Refactor common testing functions into a separate module.
- Populate transactions via file upload.
  - Add form for attaching bank statements as csv files.
    - Only accept files with the CSV extension.
    - Processing may take a while, so add a loading spinner.
    - Display an error message if the file is not a valid csv or something else
      goes wrong.
    - On success, redirect to the transactions page.
  - Add to database:
    - UPDATE: Table for transactions:
      - ADD: Import ID: NULLABLE INTEGER
          Used to detect duplicate imports. Null indicates manual entry.
    - CREATE: Table for bank/credit card accounts with:
      - Numeric ID: INTEGER
      - Account/card number: TEXT
    - CREATE: Table for balances with:
      - Account/card ID: Integer
      - Date: TEXT
      - Balance: REAL
  - Add route for reading csv files and populating transactions.
    - Handle CSV files from:
      - ASB bank accounts
      - ASB credit cards
      - Kiwibank accounts
    - Return error if the file is not one of the above formats.
    - For both credit cards and bank accounts, extract the following fields:
      - Date
      - Description
      - Amount
      - Account number
    - For bank accounts, extract:
      - Balance
    - Hash the above fields to create a unique transaction ID.
    - Reject transactions that are already in the database.
