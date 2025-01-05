# To Do

- Use semantic status codes.
  - Currently status code 200 is returned no matter what.
    This makes unit tests harder to understand.
    Adding a small JavaScript script should be enough to allow other status
    codes to swap content with HTMX.
- Prefix API endpoints with '/api' to distinguish between pages and fragments.
- Update unit tests to parse HTML document tree for checking for the existence
  of nodes and attributes.
- Add logging middleware that logs incoming requests and outgoing responses.
  Passwords should be redacted.
