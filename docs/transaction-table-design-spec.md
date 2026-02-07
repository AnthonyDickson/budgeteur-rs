# Design Specification: Transaction Table View

## Overview

The Transactions page presents a tabular view of all transactions with pagination, quick actions, and tag context. It uses the shared table styles and navigation layout across the app while keeping the primary workflow (scan -> edit/delete -> paginate) fast and predictable.

## Goals

- **Primary**: Provide a scannable, ledger-style view of transactions.
- **Secondary**: Enable quick transaction edits and deletions from the list.
- **Tertiary**: Keep the layout consistent with other list views (accounts, tags, rules).

## Information Architecture

```
Transactions Page
├── Navigation Bar
└── Page Container
    ├── Header Row
    │   ├── Title: "Transactions"
    │   ├── Link: "Import Transactions"
    │   └── Link: "Create Transaction"
    ├── Table
    │   ├── Header Row
    │   └── Body Rows (0..N)
    └── Pagination (only when rows exist)
```

## Visual Design

### Page Container

- Centered content with padding and light/dark text colors.
- Uses shared container styling for consistent spacing with other pages.

### Header Row

```
Transactions                                      Import Transactions   Create Transaction
```

- Title uses `text-xl` and `font-bold` for hierarchy.
- Action links align on the same line with wrap support.
- Links share the common link styling for consistency.

### Table Styling

Shared styles (from `src/html.rs`) provide the base look:

- Table header: uppercase, smaller text, light gray background.
- Table rows: white background with separators; dark mode inverse colors.
- Table cells: consistent horizontal/vertical padding.

### Column Layout

```
|   Amount | Date       | Description                | Tags     | Actions        |
|  $45.12  | 2025-10-05 | Coffee and bagel...        | Food     | Edit  Delete  |
```

1. **Amount**
   - Right-aligned.
   - Formatted with `format_currency` for locale-style currency.
2. **Date**
   - Date display in the application’s standard `Date` format.
3. **Description**
   - Truncated to 32 graphemes with ellipsis to prevent table overflow.
   - Full description available via `title` tooltip when truncated.
4. **Tags**
   - Tag badge if present; otherwise a muted “-”.
5. **Actions**
   - Inline “Edit” link.
   - Inline “Delete” button styled as link-like destructive action.

## Interaction Design

### Editing

- “Edit” navigates to the edit page for the transaction.
- A `redirect_url` is appended so the user returns to the same page and page size after saving.

### Deleting

```
[Delete] -> confirm dialog -> row removed
```

- “Delete” uses HTMX with a confirmation prompt.
- On confirm, the row is removed from the table (`hx-swap="outerHTML"` with `hx-target="closest tr"`).
- Errors target the global alert container.

### Pagination

```
Back  1  2  3  ...  8  9  Next
```

- Displayed only when there are rows.
- Centered pagination bar with page links, “Back”, “Next”, and ellipsis.
- Current page is bold and marked with `aria-current="page"`.
- Page size uses the configured default and is preserved across navigation.

### Empty State

```
| Amount | Date | Description | Tags | Actions |
| Nothing here yet.                         |
```

- When no transactions exist, the table body shows a single row with “Nothing here yet.”
- Pagination is hidden in the empty state.

## Data Ordering

- Transactions are ordered by date descending (newest first).
- IDs are used as a secondary sort to keep ordering stable after updates.

## Responsiveness

- Header actions wrap when needed (`flex-wrap`).
- Table remains full width with smaller text (`text-sm`) to preserve readability.

## Accessibility Notes

- Pagination includes `aria-current` for the active page.
- Truncated descriptions expose full text via `title`.
- Destructive actions include confirmation dialog text with transaction description.

## Source of Truth

- UI: `src/transaction/transactions_page.rs`
- Shared styles: `src/html.rs`

---

**Document Version:** 1.0
**Last Updated:** 2026-02-07
**Status:** Implemented
**Changes from v0.1:** Initial version
