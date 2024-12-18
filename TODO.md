# To Do

- Merge error types into single app-wide error type
- Use semantic status codes.
  - Currently status code 200 is returned no matter what.
    This makes unit tests harder to understand.
    Adding a small JavaScript script should be enough to allow other status
    codes to swap content with HTMX.
- Prefix API endpoints with '/api' to distinguish between pages and fragments.
