# Design Specification: Transaction Table View

## Overview

The Transactions page presents a tabular view of transactions with range-based navigation, interval grouping, quick actions, tag context, and an excluded-tag filter for summary totals. It uses the shared table styles and navigation layout across the app while keeping the primary workflow (scan -> edit/delete -> jump ranges) fast and predictable.

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
    ├── Range Navigation (top)
    ├── Control Cluster (range, interval, summary toggle)
    ├── Table
    │   ├── Header Row
    │   └── Body Rows (0..N)
    ├── Range Navigation (bottom)
    └── Excluded Tags Filter
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

### Range Navigation

- Displayed above and below the table when the dataset has any transactions.
- Three-column layout with previous range, current range label, and next range.
- When not on the latest range, a “Latest” link appears beneath the range label.

### Control Cluster

- Placed below the top range navigation.
- Contains three controls:
  - **Range** presets (Week, Fortnight, Month, Quarter, Half-year, Year).
  - **Interval** presets (Week, Fortnight, Month, Quarter, Half-year, Year).
  - **Summary toggle** button with a status dot and “Summary on/off” label.
- Disabled presets are gray and include a tooltip explaining why they are disabled.

### Column Layout

| Amount | Date (visually hidden) | Description         | Tags | Actions     |
| ------ | ---------------------- | ------------------- | ---- | ----------- |
| $45.12 | 2025-10-05             | Coffee and bagel... | Food | Edit Delete |

1. **Amount**
   - Right-aligned.
   - Formatted with `format_currency` for locale-style currency.
2. **Date (visually hidden)**
   - The date column is present for accessibility but hidden visually (`sr-only`).
   - Date context is provided by the date interval and day headers in grouped views.
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

The table supports grouped views inspired by the History screens in [Budgeteur](https://github.com/AnthonyDickson/budgeteur).
These views layer on top of the existing table and keep actions consistent.

### Grouping Modes

1. **Date Interval (History-style, default)**
   - Groups transactions into week intervals with a range label.
   - Within each interval, transactions are grouped by day.
   - The smallest interval size is weekly; daily intervals are not a supported mode.

2. **Category Summary (optional, layered on date intervals)**
   - Within each date interval, show a category/tag summary list.
   - Each category can be expanded to reveal its transactions grouped by day.
   - Includes percent-of-total indicators (e.g., “58% of total expenses”) and an “Other” row when needed.

### Date Interval Layout

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

- Date range labels always include the full four-digit year for consistency across all interval types.
- The per-row date cell remains in the DOM but is visually hidden for accessibility.

### Group Header Totals

- Each date interval header displays two figures: total income and total expenses.
- Totals are computed from all transactions in the interval, excluding transactions that match excluded tags (shared with the dashboard).
- Expenses use the standard negative sign formatting; accounting-style formatting is planned for later.

### Category Summary Layout (expandable within each date interval)

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

- Potential enhancement: optionally group expanded transactions by day or month for long ranges (e.g., yearly intervals).
```

### Interaction Notes

- The grouping view preserves existing actions (Edit/Delete) when rows are transactional.
- Date-range navigation moves across fixed ranges; grouping happens within the current range.
- Empty date intervals (weeks with no transactions) are omitted from grouped views.
- Grouping settings (date interval period and tag grouping toggle) persist across page loads.
- Category summary sections are collapsed by default and reset to collapsed on page refresh.
- Use a dedicated toggle control with a large tap target to avoid accidental expansion.
- Focus styles on the toggle control must be visible for keyboard users.
- Provide interval size controls (week/fortnight/month/quarter/half-year/year) to change grouping period.
- Group the range preset, date interval period, and tag grouping toggle together as a single control cluster.
- The excluded-tag filter updates summary totals and percentages without removing transactions from the table.

### Date-Range Navigation (Range-based)

```
Previous Range    Current Range Label    Next Range
```

- Navigation moves backward/forward by a fixed date range aligned to interval boundaries (no partial intervals).
- Range presets (and only supported range sizes): week, fortnight, month, quarter, half-year, year.
- Presets smaller than the selected interval are disabled with a tooltip explaining why.
- If the selected interval is larger than the current range, auto-select the smallest preset that can contain the interval.
- Current range label reflects the active range (full four-digit years).
- Date-range navigation loads a complete set of intervals within the selected range (no interval splitting).
- When the current range is not the latest, show a “Latest” link beneath the range label to allow navigation even if there are no adjacent ranges.

### Empty State (Range-based)

```
| Amount | Date | Description | Tags | Actions |
| No transactions in this range.           |
```

- When the current range has no transactions, the table body shows a single row with “No transactions in this range.”
- When summary mode has no rows due to exclusions, the table body shows “No transactions in this summary after exclusions.”
- Range navigation is shown if any transactions exist in the dataset, even when the current range is empty. It is hidden only when there are no transactions at all.

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
- Summary sections use native `<details>` / `<summary>` elements for keyboard and screen reader support.

## Excluded Tags Filter

- Rendered as a titled block (“Filter Out Tags”) below the table.
- Displays tags in a checkbox grid; excluded tags are checked on load.
- Changing a checkbox updates summary totals/percentages and keeps the user on the same range.

## Potential Enhancements

- Persist grouping settings in user preferences; allow query params to override.
- After importing transactions, advance the date range to include the latest data.
- Add filtering scoped to the current date range.
- Move full-text search to a dedicated page to allow arbitrary time ranges.

## Source of Truth

- UI: `src/transaction/transactions_page.rs`
- Rendering: `src/transaction/view.rs`
- Shared styles: `src/html.rs`

---

**Document Version:** 1.3
**Last Updated:** 2026-02-13
**Status:** In Progress
**Changes from v1.2:** Documented control cluster, latest link, excluded-tag filter, and summary empty state
