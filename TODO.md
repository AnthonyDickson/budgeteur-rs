# To Do

- CI/CD:
  - Add checks to PRs merging into main that ensure that the version has been
    increased if source files have been changed.
  - Add step in Docker image build that runs tailwind to build the CSS files.
    This should be a another stage.
- Add develop branch
  - Maintain linear history of all commits
  - Cannot be deleted
- Update unit tests to parse HTML document tree for checking for the existence
  of nodes and attributes.
- Move route handler code for views to routes/views module.
- Reorganise tailwindcss code to use partials and/or custom styles instead of
  HTML templates.
  Refer to [v3.tailwindcss.com/docs/reusing-styles](https://v3.tailwindcss.com/docs/reusing-styles) and [v3.tailwindcss.com/docs/adding-custom-styles](https://v3.tailwindcss.com/docs/adding-custom-styles).
- Fix broken [icon](./static/seal.png).
- Refactor common testing functions into a separate module.
