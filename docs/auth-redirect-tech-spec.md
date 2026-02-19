# Auth Redirect Tech Spec

## Purpose

Preserve the user's previous page when automatic logout occurs, so after re-authentication they return to where they left off instead of always landing on the dashboard.

## Scope

- Applies to automatic logout triggered by authentication middleware (missing/invalid/expired auth cookies).
- Covers both standard requests and HTMX requests.
- Covers log-in page rendering and log-in POST redirect target selection.

Out of scope:
- Manual log out flow (`GET /api/log_out`) remains unchanged.
- Cross-origin or external redirects are not supported.

## Current Behavior

- When the auth cookie is missing/invalid/expired, middleware redirects to `/log_in` (or `HX-Redirect: /log_in` for HTMX).
- On successful log-in, the user is redirected to `/dashboard`.

## Desired Behavior

- When middleware denies access, redirect to `/log_in?redirect_url=<current_path_and_query>`.
- The log-in form preserves `redirect_url` and submits it with the POST.
- On successful log-in, redirect to `redirect_url` when it is present and valid.
- If `redirect_url` is missing or invalid, fall back to `/dashboard`.

## URL/Redirect Rules

- `redirect_url` must be a relative path that starts with `/`.
- Reject URLs that start with `//` to avoid scheme-relative redirects.
- Exclude `/log_in` from valid redirect targets to prevent loops.
- No whitelist is used; any relative path that passes validation is allowed.
- If invalid or missing, ignore and use the default fallback.
- The value should represent the full path and query of the requested page (e.g. `/transactions?range=month&anchor=2025-10-05`).

## UX Notes

- The log-in page should not visibly change unless necessary.
- If a `redirect_url` is present, it is passed via a hidden form input.

## Implementation Notes

### Auth middleware

- In `src/auth/middleware.rs`, build the redirect URL using the incoming request URI:
  - If the request path starts with `/api`:
    - Require `HX-Request: true`.
    - Prefer `HX-Current-URL` as the redirect target when valid.
    - If missing or invalid, fall back to `/dashboard`.
  - For non-`/api` requests, use the request `path_and_query` as the redirect target.
  - URL-encode the redirect target as the `redirect_url` query parameter on `/log_in`.
- Apply the same logic for both:
  - `auth_guard` (standard redirect response).
  - `auth_guard_hx` (HTMX redirect header).

### Log-in page

- In `src/auth/log_in.rs`, accept an optional `redirect_url` query parameter for the GET request.
- If valid, include it in the log-in form as a hidden input so it is sent with the log-in POST.

### Log-in POST

- In `src/auth/log_in.rs`, read `redirect_url` from the form only.
- On success:
  - If valid: redirect to it.
  - Else: redirect to `/dashboard`.

### Validations & Helpers

- Centralize redirect URL validation in a small helper function (e.g., `is_safe_redirect_url`).
- Reuse the helper in middleware and log-in handlers.
- Log when a redirect URL is malformed or rejected (including missing/invalid `HX-Current-URL` on `/api` requests).

## Tests

- Middleware redirects include the expected `redirect_url` for:
  - Standard requests (Location header).
  - HTMX requests (HX-Redirect header).
- Log-in page renders a hidden `redirect_url` input when present and valid.
- Log-in POST redirects to `redirect_url` when valid; otherwise falls back to `/dashboard`.
- Invalid `redirect_url` cases (e.g., `https://example.com`, `//evil.com`, empty string) fall back to `/dashboard`.

## Risks & Mitigations

- Open redirect risk: mitigated by strict relative URL validation.
- Redirect loops to `/log_in`: mitigated by excluding `/log_in` from valid redirect targets.

## Decisions

- HTMX redirects keep `StatusCode::OK` so `HX-Redirect` is processed; HTMX does not process response headers on 3xx
  responses. <https://htmx.org/headers/hx-redirect/>

---

**Document Version:** 0.1
**Last Updated:** 2026-02-19
**Status:** Draft
