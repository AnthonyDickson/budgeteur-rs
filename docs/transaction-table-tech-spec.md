# Technical Specification: Transaction Table View (Grouping)

## Purpose

Capture non-obvious decisions and constraints in the transaction table view, including range-based navigation, interval grouping, category summaries, and excluded-tag filtering.

## Baseline (Existing Implementation)

- **Route handler**: `get_transactions_page` in `src/transaction/transactions_page.rs`.
- **Navigation**: Range-based navigation using query params `range`, `interval`, `anchor`, and `summary`.
- **Grouping**: Interval grouping and optional category summary mode.
- **Ordering**: Date descending, then ID ascending for stability after edits.
- **Rendering**: Server-side table rows using Maud templates in `src/transaction/view.rs`.
- **Actions**: Edit uses redirect URL; delete is HTMX row-local.

## Grouping Model

Grouping is layered rather than mutually exclusive:

1. **Date Interval (History-style, default)**
   - Group transactions by a fixed time range (week by default).
   - Each group header shows a date range (e.g., `2 Sep - 8 Sep 2024`).
   - Within a group, transactions are ordered by date descending then ID ascending.
   - The smallest interval size is weekly; daily intervals are not a supported mode.

2. **Category Summary (Grouped Totals, optional)**
    - Within each date interval, show a category breakdown list (tag totals + % of total income/expenses).
    - Income categories are ordered by total descending; expense categories are ordered by absolute total descending.
    - Include an “Other” row for `None` tags.
    - Each category row can expand to reveal transactions grouped by day using the same transaction row layout.

The UI can toggle the category summary layer on/off without changing the underlying query semantics.

## Non-Obvious Decisions

### Range navigation vs grouping

- **Decision**: Range first, then group within the range.
- **Rationale**: Keeps query cost low and preserves range navigation semantics; avoids multi-range group headers.
- **Tradeoff**: A group can be split across ranges only if the interval is larger than the range; presets prevent this.

### Group ordering

- **Decision**: Order groups by their most recent transaction date (desc). Within groups, sort by date desc + ID asc.
- **Rationale**: Matches the user’s expectation that “newer stuff is higher” and preserves stability on edits.

### Summary calculation scope

- **Decision**: Category summary totals are computed from the transactions in the current range only.
- **Rationale**: Keeps summaries consistent with what the user sees without requiring full dataset scans.

### Empty intervals

- **Decision**: Omit empty date intervals (no transactions) from the grouped views.

### Group header totals

- **Decision**: Interval headers show total income and total expenses as separate figures.
- **Decision**: Totals are computed from all transactions in the interval, excluding transactions with excluded tags (shared tag filter used by dashboard + transaction table).
- **Decision**: Use negative sign formatting for expenses; accounting-style formatting is planned for later.

### Excluded tag filter

- **Decision**: Pull the excluded-tag filter out of the dashboard feature into a shared feature used by both dashboard and transaction table.
- **Decision**: Excluded tags affect interval totals and summary calculations but do not hide transactions from the table.

### Truncation logic unchanged

- Description truncation remains grapheme-based (32 graphemes) with tooltips for full text.

### Grouped view row affordances

- Grouped layouts should preserve the existing row affordances from the baseline table view (inline actions, tag display).
- The date column remains in the DOM but is visually hidden (`sr-only`) to avoid redundancy with day headers while keeping screen reader context.

### Collapsible sections

- Prefer native HTML5 disclosure elements (`<details>`/`<summary>`) for collapsible UI.
- Avoid custom JS unless a specific interaction is not achievable with native elements.
- Preserve accessibility defaults (keyboard and screen reader affordances) when styling.
- Grouping settings (date interval period and tag grouping toggle) persist across page loads via query params.
- Category summary sections are collapsed by default and reset to collapsed on page refresh.
- Use a dedicated toggle control with a large tap target to avoid accidental expansion.
- Ensure visible focus styling on the toggle control for keyboard users.

### Edit redirect continuity

- Grouping must preserve the `redirect_url` query parameter to return users to the same range, interval, and summary mode.

## Data Requirements

### Existing fields (already available)

- `amount`, `date`, `description`, `tag_name`, `tag_id`, `id`

### Additional fields (optional)

- `import_id` if future grouping by import batch is desired.

### Tag model

- Each transaction has at most one tag by design.
- Untagged transactions are grouped under an "Other" tag in category summary views (matching dashboard aggregations).

## UI Structure (Grouped)

### Date Interval View

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

### Category Summary View (expandable within each date interval)

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

- Potential enhancement: optionally group expanded transactions by day or month for long ranges (e.g., yearly intervals).
- The implemented view groups expanded transactions by day (day headers reuse the same day header styling as the main table).

### Empty States

- When the current range has no transactions, render a single empty row with “No transactions in this range.”
- When summary mode is enabled and all rows are excluded, render “No transactions in this summary after exclusions.”
- Range navigation is shown whenever the dataset has any transactions, even if the current range is empty.

## Calculation Notes

- **Date interval boundaries**:
  - Week: Monday–Sunday.
  - Fortnight: 1–14 and 15–end of month (calendar-aligned, not rolling 14-day windows).
  - Month/Quarter/Half-year/Year: calendar-aligned ranges.
- **Timezone**: Default anchor date uses the server’s configured local timezone; range math uses `Date` only.
- **Range label format**: Always include the full four-digit year in date range labels for consistency across all interval types.
- **Percentages**: Compute as `category_total / total_income * 100` for income categories and `category_total / total_expenses * 100` for expense categories, rounded to the nearest integer. Totals and category values are negative for expenses, yielding positive percentages.
- **Income vs expenses**: In category summary, split per tag into income and expense sections when both exist.

## Error Handling

- Maintain existing HTMX delete error target (`#alert-container`).
- Grouping should not change error handling semantics.

## Testing Expectations

- Keep existing range navigation tests passing.
- Add tests for:
  - Group headers rendering with correct range labels.
  - Category summary rows with percent calculations.
  - Ordering of groups and items.
  - Summary empty state copy when all rows are excluded.
  - Latest link visibility when not on latest range.
  - Excluded tag filter controls rendering.

## Source of Truth

- Handler and rendering: `src/transaction/transactions_page.rs`
- View templates: `src/transaction/view.rs`
- Shared styles: `src/html.rs`

---

**Document Version:** 0.3
**Last Updated:** 2026-02-13
**Status:** Draft
**Changes from v0.2:** Documented range/interval boundaries, excluded-tag behavior, and summary empty state

## Range-based Grouping + Date-Range Navigation

### Date-Range Navigation Model

- **Decision**: Use date ranges aligned to interval boundaries.
- **Decision**: Range presets (and only supported range sizes): week, fortnight, month, quarter, half-year, year.
- **Decision**: Presets smaller than the selected interval are disabled with a tooltip explaining why.
- **Decision**: If the selected interval is larger than the current range, auto-select the smallest preset that can contain the interval.
- **Decision**: Navigation moves by the range size (based on the preset), not by the interval size.
- **Decision**: When the current range is not the latest, show a “Latest” link that jumps to the newest range.
- **Decision**: Prev/next anchors use the day before the current range start and the day after the current range end; links encode the range preset and anchor date.
- **Decision**: Latest link is computed from the latest transaction date in the dataset (bounds), using the same range preset.

### Query Parameters

- **Decision**: Encode the range preset and anchor date in query params; defaults apply when absent.
- **Decision**: Encode the interval preset in query params; default to weekly intervals when absent.
- **Decision**: Encode the category summary toggle as `summary=true` in query params.
- **Decision**: Default anchor date uses the user’s local timezone (`get_local_offset`).
- **Decision**: If the requested range preset cannot contain the selected interval, redirect to the smallest compatible range preset (HTTP 303).

### Grouping Scope (Range-based)

- **Decision**: Fetch a complete set of intervals within the selected date range, then group within that range.
- **Decision**: Category summary totals are computed from the transactions within the current date range.

### Redirect Continuity (Range-based)

- **Decision**: Preserve range preset + anchor date + interval + summary in `redirect_url`, excluding excluded-tag preferences (saved independently).

### Import Behavior

- **Decision**: After importing transactions, advance the date range to include the latest data.

### Accessibility (Range-based)

- **Decision**: Ensure all table actions and controls are keyboard-navigable.

### Potential Enhancements (Range-based)

- Persist grouping settings in user prefs; allow query params to override.
- Add filtering scoped to the current date range.
- Move full-text search to a dedicated page to allow arbitrary time ranges.
- After importing transactions, advance the date range to include the latest data.
