# Technical Specification: Expenses by Tag Cards

## Overview

Implementation guide for the card-based expense breakdown feature on the dashboard page.

**Related Documents:**

- Design Specification: `expenses-by-tag-design-spec.md`
- Existing Code: `handlers.rs`, `tables.rs`, `aggregation.rs`, `html.rs`

---

## Architecture

### Module Structure

```
dashboard/
├── mod.rs                  (public exports)
├── handlers.rs             (HTTP handlers) ⬅️ MODIFY
├── tables.rs               (table components) 
├── charts.rs               (chart components)
├── cards.rs                (NEW - card components)
├── aggregation.rs          (data processing) ⬅️ MODIFY
├── transaction.rs          (database queries)
└── preferences.rs          (user preferences)
```

### Data Flow

```
Handler (get_dashboard_page)
    ↓
Query DB (get_transactions_in_date_range)
    ↓
Aggregate (calculate_tag_statistics)
    ↓
Render (expense_cards_view)
    ↓
HTML Response
```

---

## Database Schema

### No Changes Required

Existing schema already supports this feature:

- `transaction` table has `tag_id` and `amount`
- `tag` table has tag names (user can include emojis)
- `dashboard_excluded_tags` for filtering

---

## Data Structures

### New Type: `TagExpenseStats` (in `cards.rs`)

```rust
struct TagExpenseStats {
    tag: String,                    // Tag name (as entered by user, may include emoji)
    current_month_amount: f64,      // This month's expenses for tag
    percentage_of_total: f64,       // % of total expenses
    monthly_average: f64,           // Average over available months
    percentage_change: f64,         // % change from average
    annual_delta: f64,              // (current - average) * 12
    months_of_data: usize,          // How many months of data exist
}

enum CardState {
    Overspending,         // ≥5% above average
    Saving,               // ≥5% below average
    OnTrack,              // <5% variance
    InsufficientData,     // <1 month
}

impl TagExpenseStats {
    fn card_state(&self) -> CardState {
        if self.months_of_data < 1 {
            CardState::InsufficientData
        } else if self.percentage_change.abs() < 5.0 {
            CardState::OnTrack
        } else if self.percentage_change >= 5.0 {
            CardState::Overspending
        } else {
            CardState::Saving
        }
    }
}
```

---

## Implementation Steps

### Step 1: Add Data Aggregation Function

**File:** `aggregation.rs`

**New function:**

```rust
pub(super) fn calculate_tag_expense_statistics(
    transactions: &[Transaction],
    current_month: Date,
) -> Vec<TagExpenseStats>
```

**Algorithm:**

1. Filter transactions to expenses only (amount < 0)
2. Group by tag name
3. For each tag:
   - Calculate current month total
   - Calculate monthly average over period
   - Calculate percentage of total expenses
   - Calculate percentage change from average
   - Calculate annual delta: `(current - average) * 12`
   - Count months of data available
4. Sort by current month amount (descending)
5. Move "Other" tag to end

**Pseudo-code:**

```
function calculate_tag_expense_statistics(transactions, current_month):
    expenses = filter(transactions where amount < 0)
    grouped = group_by_tag(expenses)
    total_expenses = sum(map(expenses, |t| t.amount.abs()))
    
    stats = []
    for tag, tag_transactions in grouped:
        current = sum_for_month(tag_transactions, current_month)
        total = sum(map(tag_transactions, |t| t.amount.abs()))
        unique_months = get_unique_months(tag_transactions)
        months = length(unique_months)
        average = if months > 0 then total / months else 0
        
        stat = TagExpenseStats {
            tag: tag,
            current_month_amount: current,
            percentage_of_total: (current / total_expenses) * 100,
            monthly_average: average,
            percentage_change: if average > 0 then ((current - average) / average) * 100 else 0,
            annual_delta: (current - average) * 12,
            months_of_data: months,
        }
        stats.push(stat)
    
    sort(stats, by current_month_amount descending)
    move_to_end(stats, where tag == "Other")
    return stats
```

**Tests:**

- Basic calculation correctness
- "Other" tag sorted to end
- Positive amounts excluded
- Empty input handling
- Insufficient data detection (<1 month)
- Small change (<5%) = OnTrack
- All tags shown regardless of size

---

### Step 2: Create Card Rendering Module

**File:** `cards.rs` (NEW)

**Imports needed:**

```rust
use maud::{Markup, html};
use crate::html::{format_currency, LINK_STYLE};
use crate::endpoints;
```

**Main function:**

```rust
pub(super) fn expense_cards_view(tag_stats: &[TagExpenseStats]) -> Markup
```

**Helper functions:**

```rust
fn expense_card(stat: &TagExpenseStats) -> Markup
fn card_bottom_content(stat: &TagExpenseStats, state: CardState) -> Markup
fn trend_indicator(stat: &TagExpenseStats) -> Markup
fn progress_bar(percentage: f64) -> Markup
fn empty_state_view() -> Markup
fn helper_card() -> Markup
```

**Structure:**

```
expense_cards_view
├── If empty: return empty_state_view()
├── Section header
│   ├── "Expenses by Tag" (h3)
│   └── "Last 12 months" (subtitle)
└── Grid of cards
    ├── For each tag: expense_card()
    └── If ≤2 tags: helper_card()

expense_card
├── Tag name (includes emoji if user added one)
├── Current amount (large, use format_currency)
├── Percentage of total
├── Progress bar
└── card_bottom_content (based on state)

card_bottom_content (varies by state)
├── InsufficientData: "Building baseline..."
├── OnTrack: Average + "→ On track"
├── Overspending: Average + trend (red) + delta (red)
└── Saving: Average + trend (green) + delta (green) + 🎉
```

**Key implementation notes:**

- Use `format_currency()` from `html.rs` for all monetary displays
- Use `LINK_STYLE` from `html.rs` for the "Manage Tags →" link
- Tag name displays exactly as stored - no emoji mapping needed
- Progress bar uses existing dark mode color scheme
- All cards shown regardless of size - no filtering by percentage

**Tests:**

- Renders overspending card correctly
- Renders saving card with celebration
- Renders on-track card without delta
- Renders insufficient data card
- Empty state when no tags
- Helper card when few tags
- Tag name displays with emoji if present

---

### Step 3: Update Handlers

**File:** `handlers.rs`

**Modify:** `get_dashboard_page`

```rust
// After getting transactions, before rendering:
let today = OffsetDateTime::now_utc().to_offset(local_timezone).date();
let current_month = today.replace_day(1).unwrap();

let tag_stats = calculate_tag_expense_statistics(
    &transactions,
    current_month,
);

// Pass tag_stats to view
dashboard_view(nav_bar, tags_with_status, charts, tables, tag_stats)
```

**Modify:** `dashboard_view` signature

```rust
fn dashboard_view(
    nav_bar: NavBar,
    tags_with_status: &[TagWithExclusion],
    charts: &[DashboardChart],
    tables: &[Markup],
    tag_stats: &[TagExpenseStats],  // NEW
) -> Markup
```

**Add to HTML:**

```rust
// After charts/tables grid, before tag filter:
(expense_cards_view(tag_stats))
```

**Modify:** `update_excluded_tags` handler similarly

- Recalculate tag_stats after filtering
- Pass to `dashboard_content_partial`

**Modify:** `dashboard_content_partial` signature and call

---

### Step 4: Update Module Exports

**File:** `mod.rs`

```rust
mod cards;  // Add this line

// Rest unchanged
```

---

## Testing Strategy

### Unit Tests

**aggregation.rs:**

- ✅ Calculates amounts, percentages, deltas correctly
- ✅ Sorts by amount with "Other" at end
- ✅ Excludes positive net amounts
- ✅ Handles insufficient data (<1 month)
- ✅ Handles small changes as OnTrack
- ✅ Empty input returns empty vector
- ✅ All tags included regardless of size

**cards.rs:**

- ✅ Renders each card state correctly
- ✅ Shows empty state when no tags
- ✅ Includes helper card when ≤2 tags
- ✅ Progress bar width calculated correctly
- ✅ Currency formatting uses format_currency()
- ✅ Tag name displayed as-is (with emoji if present)
- ✅ Link uses LINK_STYLE constant

### Integration Tests

**handlers.rs:**

```rust
#[tokio::test]
async fn dashboard_shows_expense_cards() {
    // Create transactions with tags (some with emoji)
    // Call get_dashboard_page
    // Assert cards section exists in HTML
    // Assert correct card count
}

#[tokio::test]
async fn expense_cards_update_when_tags_excluded() {
    // Create tagged transactions
    // Exclude a tag
    // Verify cards reflect filtered data
}
```

---

## Performance Considerations

### Query Performance

- No new database queries needed
- Reuse existing `get_transactions_in_date_range` result
- In-memory aggregation is O(n) where n = transactions
- Typical case: ~500-2000 transactions/year, negligible

### Rendering Performance

- Card rendering is O(m) where m = number of tags
- Typical case: ~10 tags, negligible impact
- Pure server-side rendering (no JS overhead)

---

## Accessibility Requirements

- [ ] Each card has descriptive `aria-label`
- [ ] Progress bars: `role="progressbar"` with aria attributes
- [ ] Color + arrows (not color alone) for trends
- [ ] Test with screen reader
- [ ] Verify WCAG contrast ratios

---

## Error Handling

### Graceful Degradation

- Database errors: Existing error propagation handles this
- Calculation errors: Guard against division by zero
- Rendering errors: Show empty state on failure

### Edge Cases Handled

- No transactions → existing dashboard logic
- No tags → empty state with helper text
- Single tag → card + helper
- Zero division → guarded in percentage calculations
- Large deltas → show as calculated, no special handling

---

## Deployment Checklist

- [ ] All tests passing (`cargo test`)
- [ ] No clippy warnings (`cargo clippy`)
- [ ] Code formatted (`cargo fmt`)
- [ ] Manual testing in dev
- [ ] Test responsive behavior (mobile/tablet/desktop)
- [ ] Verify dark mode
- [ ] Check accessibility with screen reader

---

## Configuration

### Constants

```rust
const MIN_CHANGE_THRESHOLD: f64 = 5.0;  // % change for significant variance
```

### No Feature Flags Needed

This is purely additive with no toggle required.

---

## Migration & Rollback

### Migration

**None required.** This is purely additive.

### Rollback Plan

1. **Quick:** Comment out `expense_cards_view()` call in handlers
2. **Full:** Revert commits for `cards.rs` and handler modifications
3. **Zero data risk:** No database changes

---

## Future Enhancements

Potential additions:

- Sort options (by amount, %, delta)
- Click card → filter transactions by tag
- Hover tooltip with monthly breakdown
- Card animations on load
- Loading skeletons during HTMX updates

---

## Dependencies

### Crate Requirements

**No new dependencies.** Uses existing:

- `maud` - HTML templating
- `time` - Date handling
- `rusqlite` - Database

### Version Requirements

- Rust 1.70+ (existing requirement)
- No `Cargo.toml` changes needed

---

## Code Style Guidelines

### Follow Existing Patterns

- Use `pub(super)` for module-internal visibility
- Document public functions with `///` doc comments
- Use `#[derive(Debug, Clone)]` where appropriate
- Error handling via `Result<T, Error>` propagation
- Use `tracing::error!` for logging
- Follow existing naming: snake_case functions, PascalCase types

### Use Existing HTML Utilities

- **Currency formatting:** Always use `format_currency()` from `html.rs`
- **Links:** Use `LINK_STYLE` constant for consistent styling
- **Colors:** Use existing Tailwind classes for red/green/gray/blue
- **Dark mode:** All color classes must include dark mode variants

### Maud Conventions

- Use `html!` macro for markup
- Keep view functions pure (no side effects)
- Extract repeated patterns into helper functions
- Match existing card/table styling patterns

---

## Documentation

### Code Comments

- Document complex calculations (especially aggregation logic)
- Explain edge case handling
- Reference design spec for UX decisions

### No User Documentation Needed

- Feature is self-explanatory
- Follows existing dashboard patterns

---

## References

- Design Spec: `expenses-by-tag-design-spec.md`
- Existing Code: `handlers.rs`, `tables.rs`, `aggregation.rs`, `html.rs`
- Maud Docs: https://maud.lambda.xyz/
- Tailwind CSS: https://tailwindcss.com/

---

**Document Version:** 1.0
**Last Updated:** 2026-01-24
**Status:** Ready for Implementation
**Estimated Effort:** 4-6 hours
