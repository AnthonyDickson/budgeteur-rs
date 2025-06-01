# To Do

- Update unit tests to parse HTML document tree for checking for the existence
  of nodes and attributes.
- Move route handler code for views to routes/views module.
- Reorganise tailwindcss code to use partials and/or custom styles instead of
  HTML templates.
  Refer to [v3.tailwindcss.com/docs/reusing-styles](https://v3.tailwindcss.com/docs/reusing-styles) and [v3.tailwindcss.com/docs/adding-custom-styles](https://v3.tailwindcss.com/docs/adding-custom-styles).
- Fix broken [icon](./static/seal.png).
- Refactor common testing functions into a separate module.
- Refactor app to assume single user
  - Simpler and all I need!
  - Log in flow
    - Log in page should have link for "forgot your password?". This should
      give instructions to run a CLI script that will change the password in the
      database. The CLI script should use a similar flow to the registration
      page for setting up a password.
