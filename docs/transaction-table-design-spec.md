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

| Amount | Date (visually hidden) | Description         | Tags | Actions     |
| ------ | ---------------------- | ------------------- | ---- | ----------- |
| $45.12 | 2025-10-05             | Coffee and bagel... | Food | Edit Delete |

1. **Amount**
   - Right-aligned.
   - Formatted with `format_currency` for locale-style currency.
2. **Date (visually hidden)**
   - The date column is present for accessibility but hidden visually (`sr-only`).
   - Date context is provided by the date bucket and day headers in grouped views.
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

## Grouped Transactions

The table will support grouped views inspired by the History screens. These views layer on top of the existing table and keep pagination and actions consistent.

### Grouping Modes

1. **Date Bucket (History-style, default)**
   - Groups transactions into week buckets with a range label.
   - Within each bucket, transactions are grouped by day.
   - The smallest bucket size is weekly; daily buckets are not a supported mode.

2. **Category Summary (optional, layered on date buckets)**
   - Within each date bucket, show a category/tag summary list.
   - Each category can be expanded to reveal a flat list of its transactions (date shown per row).
   - Includes percent-of-total indicators and an “Other” row when needed.

### Date Bucket Layout

```
2 Sep - 8 Sep 2024                             $800.00
------------------------------------------------------
07 Sep
  Donation                      $7.68
  Entertainment                 $19.19
06 Sep
  Groceries                     $75.32
  Entertainment                 $2.97
```

- Date range labels always include the full four-digit year for consistency across all bucket types.
- The per-row date cell remains in the DOM but is visually hidden for accessibility.

### Group Header Totals

- Each date bucket header displays two figures: total income and total expenses.
- Totals are computed from all transactions in the bucket, excluding transactions that match excluded tags (shared with the dashboard).
- Expenses use the standard negative sign formatting; accounting-style formatting is planned for later.

### Category Summary Layout (expandable within each date bucket)

```
2 Sep - 8 Sep 2024                    $800.00  -$937.36
------------------------------------------------------
Income                                $800.00   100%   >
Home Expenses                         $542.85    58%   >
Groceries                             $294.71    31%   >
Entertainment                          $67.58     7%   >
Donation                               $32.23     3%   >

Home Expenses (expanded)
------------------------------------------------------
07 Sep  Rent                          $500.00
07 Sep  Utilities                      $42.85
06 Sep  Insurance                      $0.00

- Potential enhancement: optionally group expanded transactions by day or month for long ranges (e.g., yearly buckets).
```

### Interaction Notes

- The grouping view preserves existing actions (Edit/Delete) when rows are transactional.
- Date-range navigation moves across fixed windows; grouping happens within the current window.
- Empty date buckets (weeks with no transactions) are omitted from grouped views.
- Grouping settings (date bucket period and tag grouping toggle) persist across page loads.
- Category summary sections are collapsed by default and reset to collapsed on page refresh.
- Use a dedicated toggle control with a large tap target to avoid accidental expansion.
- Focus styles on the toggle control must be visible for keyboard users.
- Provide bucket size controls (week/fortnight/month/quarter/half-year/year) to change grouping period.
- Group the window preset, date bucket period, and tag grouping toggle together as a single control cluster.

### Date-Range Navigation (Windowed)

```
Previous Range    Current Range Label    Next Range
```

- Navigation moves backward/forward by a fixed date window aligned to bucket boundaries (no partial buckets).
- Window presets (and only supported window sizes): last week, last fortnight, last month, last quarter, last half year, last year.
- Presets smaller than the selected bucket are disabled with a tooltip explaining why.
- If the selected bucket is larger than the current window, auto-select the smallest preset that can contain the bucket.
- Current range label reflects the active window (full four-digit years).
- Date-range navigation loads a complete set of buckets within the selected window (no bucket splitting).

### Empty State (Windowed)

```
| Amount | Date | Description | Tags | Actions |
| No transactions in this range.           |
```

- When no transactions exist, the table body shows a single row with “Nothing here yet.”
- Date-range navigation is hidden in the empty state.

## Data Ordering

- Transactions are ordered by date descending (newest first).
- IDs are used as a secondary sort to keep ordering stable after updates.

## Responsiveness

- Header actions wrap when needed (`flex-wrap`).
- Table remains full width with smaller text (`text-sm`) to preserve readability.

## Accessibility Notes

- Date-range navigation includes `aria-current` for the active range.
- Truncated descriptions expose full text via `title`.
- Destructive actions include confirmation dialog text with transaction description.

## Potential Enhancements

- Persist grouping settings in user preferences; allow query params to override.
- After importing transactions, advance the date window to include the latest data.
- Add filtering scoped to the current date window.
- Move full-text search to a dedicated page to allow arbitrary time ranges.

## Source of Truth

- UI: `src/transaction/transactions_page.rs`
- Shared styles: `src/html.rs`

---

**Document Version:** 1.2
**Last Updated:** 2026-02-11
**Status:** In Progress
**Changes from v1.1:** Folded windowed navigation into baseline; updated empty state copy
