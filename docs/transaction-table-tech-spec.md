# Technical Specification: Transaction Table View (Grouping)

## Purpose

Capture non-obvious decisions and constraints in the transaction table view while defining the initial approach for grouped rendering inspired by the provided History screens.

## Baseline (Existing Implementation)

- **Route handler**: `get_transactions_page` in `src/transaction/transactions_page.rs`.
- **Pagination**: Query params `page` and `per_page` with defaults from `PaginationConfig`.
- **Ordering**: Date descending, then ID ascending for stability after edits.
- **Rendering**: Server-side table rows using Maud templates.
- **Actions**: Edit uses redirect URL; delete is HTMX row-local.

## Grouping Model (New)

Grouping is layered rather than mutually exclusive:

1. **Date Bucket (History-style, default)**
   - Group transactions by a fixed time range (week by default).
   - Each group header shows a date range (e.g., `2 Sep - 8 Sep 2024`).
   - Within a group, transactions are ordered by date descending then ID ascending.
   - The smallest bucket size is weekly; daily buckets are not a supported mode.

2. **Category Summary (Grouped Totals, optional)**
   - Within each date bucket, show a category breakdown list (tag totals + % of total expenses).
   - Items are ordered by amount descending (largest category first).
   - Include an “Other” row for `None` tags.
   - Each category row can expand to reveal a flat list of its transactions (date shown per row).

The UI can toggle the category summary layer on/off without changing the underlying query semantics.

## Non-Obvious Decisions

### Pagination vs grouping

- **Decision**: Page first, then group within the page.
- **Rationale**: Keeps query cost low and preserves current pagination semantics; avoids multi-page group headers.
- **Tradeoff**: A group can be split across pages. This is acceptable for now and can be revisited later.

### Group ordering

- **Decision**: Order groups by their most recent transaction date (desc). Within groups, sort by date desc + ID asc.
- **Rationale**: Matches the user’s expectation that “newer stuff is higher” and preserves stability on edits.

### Summary calculation scope

- **Decision**: Category summary totals are computed from the transactions on the current page only.
- **Rationale**: Keeps summaries consistent with what the user sees without requiring full dataset scans.

### Empty buckets

- **Decision**: Omit empty date buckets (no transactions) from the grouped views.

### Group header totals

- **Decision**: Bucket headers show total income and total expenses as separate figures.
- **Decision**: Totals are computed from all transactions in the bucket, excluding transactions with excluded tags (shared tag filter used by dashboard + transaction table).
- **Decision**: Use negative sign formatting for expenses; accounting-style formatting is planned for later.

### Excluded tag filter

- **Decision**: Pull the excluded-tag filter out of the dashboard feature into a shared feature used by both dashboard and transaction table.

### Truncation logic unchanged

- Description truncation remains grapheme-based (32 graphemes) with tooltips for full text.

### Grouped view row affordances

- Grouped layouts should preserve the existing row affordances from the baseline table view (inline actions, tag display).
- The date column remains in the DOM but is visually hidden (`sr-only`) to avoid redundancy with day headers while keeping screen reader context.

### Collapsible sections

- Prefer native HTML5 disclosure elements (`<details>`/`<summary>`) for collapsible UI.
- Avoid custom JS unless a specific interaction is not achievable with native elements.
- Preserve accessibility defaults (keyboard and screen reader affordances) when styling.
- Grouping settings (date bucket period and tag grouping toggle) persist across page loads.
- Category summary sections are collapsed by default and reset to collapsed on page refresh.
- Use a dedicated toggle control with a large tap target to avoid accidental expansion.
- Ensure visible focus styling on the toggle control for keyboard users.

### Edit redirect continuity

- Grouping must preserve the `redirect_url` query parameter to return users to the same page and size.

## Data Requirements

### Existing fields (already available)

- `amount`, `date`, `description`, `tag_name`, `id`

### Additional fields (optional)

- `import_id` if future grouping by import batch is desired.

### Tag model

- Each transaction has at most one tag by design.
- Untagged transactions are grouped under an "Other" tag in category summary views (matching dashboard aggregations).

## UI Structure (Grouped)

### Date Bucket View

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

### Category Summary View (expandable within each date bucket)

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
```

- Potential enhancement: optionally group expanded transactions by day or month for long ranges (e.g., yearly buckets).

## Calculation Notes

- **Date bucket boundaries**: Use ISO week (Mon-Sun) by default. This should be configurable later.
- **Timezone**: Bucket calculations use the server timezone, which is assumed to match the user’s timezone (self-hosted).
- **Range label format**: Always include the full four-digit year in date range labels for consistency across all bucket types.
- **Percentages**: Compute as `category_total / total_income * 100` for income categories and `category_total / total_expenses * 100` for expense categories, rounded to the nearest integer.
- **Income vs expenses**: In category summary, show expenses by default; include income row when present (matching screenshot).

## Error Handling

- Maintain existing HTMX delete error target (`#alert-container`).
- Grouping should not change error handling semantics.

## Testing Expectations

- Keep existing pagination tests passing.
- Add tests for:
  - Group headers rendering with correct range labels.
  - Category summary rows with percent calculations.
  - Ordering of groups and items.

## Source of Truth

- Handler and rendering: `src/transaction/transactions_page.rs`
- Shared styles: `src/html.rs`

---

**Document Version:** 0.1
**Last Updated:** 2026-02-07
**Status:** Draft
**Changes from v0.0:** Initial grouping specification


## New Feature: Windowed Grouping + Date-Range Navigation (vNext)

### Date-Range Navigation Model

- **Decision**: Replace page-based pagination with date-range windows aligned to bucket boundaries.
- **Decision**: Window presets (and only supported window sizes): last week, last fortnight, last month, last quarter, last half year, last year.
- **Decision**: Presets smaller than the selected bucket are disabled with a tooltip explaining why.
- **Decision**: If the selected bucket is larger than the current window, auto-select the smallest preset that can contain the bucket.
- **Decision**: Navigation moves by the window size (based on the preset), not by the bucket size.

### Query Parameters

- **Decision**: Encode the window preset and anchor date in query params; defaults apply when absent.

### Grouping Scope (Windowed)

- **Decision**: Fetch a complete set of buckets within the selected date window, then group within that window.
- **Decision**: Category summary totals are computed from the transactions within the current date window.

### Redirect Continuity (Windowed)

- **Decision**: Preserve window preset + anchor date + bucket + tag grouping + filters in `redirect_url`, excluding excluded-tag preferences.

### Import Behavior

- **Decision**: After importing transactions, advance the date window to include the latest data.

### Accessibility (Windowed)

- **Decision**: Ensure all table actions and controls are keyboard-navigable.

### Potential Enhancements (Windowed)

- Persist grouping settings in user prefs; allow query params to override.
- Add filtering scoped to the current date window.
- Move full-text search to a dedicated page to allow arbitrary time ranges.

### Implementation Order (vNext)

1. Extract the excluded-tag filter into a shared module used by dashboard + transactions.
2. Replace page/per_page pagination with date-range window navigation (query params + defaults + UI).
3. Implement date bucket grouping within the current window and compute header totals.
4. Add category summary layer with expandable sections and keyboard-accessible toggles.
5. Wire window/bucket interactions, preset validation, redirect continuity, and empty state text.
6. Add/adjust tests for window navigation, grouping, totals/percentages, and “Other” tag handling.

### File-Level Plan (vNext)

- **Shared excluded-tag prefs**: extract from `src/dashboard/preferences.rs` into a shared module (e.g., `src/tag/preferences.rs` or `src/tag/excluded_tags.rs`), and update `src/dashboard/handlers.rs` + `src/dashboard/transaction.rs` to use it.
- **Window query + navigation UI**: update `src/transaction/transactions_page.rs` to parse window preset + anchor date params, compute window boundaries, and render the range navigation controls.
- **Grouping + bucket totals**: add grouping helpers in `src/transaction/transactions_page.rs` or a new `src/transaction/grouping.rs` module; compute bucket totals using the shared excluded-tag filter.
- **Category summary + expand/collapse**: render category summary rows and expandable sections in `src/transaction/transactions_page.rs`; add toggle markup consistent with `<details>/<summary>` in `src/html.rs` if new shared styles are needed.
- **Preset validation + auto-select**: enforce preset/window rules in `src/transaction/transactions_page.rs`; ensure redirect URLs include window preset + anchor date + bucket + tag grouping + filters.
- **Tests**: update pagination tests in `src/transaction/transactions_page.rs` to cover window navigation, preset validation, bucket totals, percentage calculations, and “Other” grouping.
