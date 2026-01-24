# Technical Specification: Expenses by Tag Cards

## Overview

Implementation guide for the card-based expense breakdown feature on the dashboard page.

**Related Documents:**

- Design Specification: `expenses-by-tag-design-spec.md`
- Existing Code: `handlers.rs`, `tables.rs`, `aggregation.rs`, `html.rs`

**Key Change from Original Spec:** Cards display **last complete month** data, not current partial month.

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
Calculate last_complete_month (today - 1 month)
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

### Type: `TagExpenseStats` (in `aggregation.rs`)

```rust
/// Statistics for a single tag's expenses.
///
/// Compares last complete month against historical average (excluding that month).
#[derive(Debug, Clone)]
pub(super) struct TagExpenseStats {
    /// Tag name (as entered by user, may include emoji)
    pub tag: String,
    /// Last complete month's expenses for tag (absolute value)
    pub last_month_amount: f64,
    /// Percentage of total expenses (for last complete month)
    pub percentage_of_total: f64,
    /// Average monthly expenses over historical data (excluding displayed month)
    pub monthly_average: f64,
    /// Percentage change from historical average (+/- percent)
    pub percentage_change: f64,
    /// Projected annual delta: (last_month - average) * 12
    pub annual_delta: f64,
    /// Number of complete months of data available
    pub months_of_data: usize,
}
```

### Enum: `CardState` (in `cards.rs`)

```rust
enum CardState {
    Overspending,         // ≥5.5% above average
    Saving,               // ≤-5.5% below average
    OnTrack,              // <5.5% variance
    InsufficientData,     // <2 complete months
}
```

---

## Implementation Steps

### Step 1: Add Data Aggregation Function

**File:** `aggregation.rs`

**New function:**

```rust
/// Calculates expense statistics by tag for the expense cards.
///
/// Compares a specific month (typically last complete month) against historical
/// average calculated from all other months.
///
/// # Arguments
/// * `transactions` - All transactions to analyze
/// * `comparison_month` - The month to analyze (typically last complete month)
///
/// # Returns
/// Vector of tag statistics sorted by comparison month amount (descending),
/// with "Other" tag sorted to the end.
///
/// # Implementation Notes
/// - Only negative amounts (expenses) are included
/// - Historical average excludes the comparison month to avoid circular comparison
/// - Requires at least 2 complete months of data (one to display, one for baseline)
pub(super) fn calculate_tag_expense_statistics(
    transactions: &[Transaction],
    comparison_month: Date,
) -> Vec<TagExpenseStats>
```

**Algorithm:**

1. Filter transactions to expenses only (amount < 0)
2. Calculate total expenses for the comparison month (for percentage calculation)
3. Group by tag name
4. For each tag:
   - Calculate comparison month total
   - **Separate historical transactions** (all months except comparison month)
   - Calculate historical average from those months only
   - If no historical data exists (first month), mark as InsufficientData
   - Calculate percentage of total expenses (comparison month amount / total)
   - Calculate percentage change from historical average
   - Calculate annual delta: `(comparison_month - historical_average) * 12`
   - Count total months of data available
5. Sort by comparison month amount (descending)
6. Track "Other" tag separately during iteration, append at end

**Pseudo-code:**

```
function calculate_tag_expense_statistics(transactions, comparison_month):
    expenses = filter(transactions where amount < 0)
    
    # Calculate total for the comparison month (for percentage_of_total)
    comparison_month_expenses = filter(expenses where month == comparison_month)
    total_expenses = sum(map(comparison_month_expenses, |t| t.amount.abs()))
    
    grouped = group_by_tag(expenses)
    
    stats = []
    other_stat = None
    
    for tag, tag_transactions in grouped:
        # Amount for comparison month
        comparison_amount = sum(
            filter(tag_transactions where month == comparison_month)
            map(|t| t.amount.abs())
        )
        
        # Separate historical data (excluding comparison month)
        historical = filter(tag_transactions where month != comparison_month)
        
        if historical.is_empty():
            # First month - insufficient data for comparison
            monthly_average = comparison_amount
            percentage_change = 0.0
        else:
            historical_months = unique_months(historical)
            historical_total = sum(map(historical, |t| t.amount.abs()))
            monthly_average = historical_total / count(historical_months)
            
            percentage_change = if monthly_average > 0:
                ((comparison_amount - monthly_average) / monthly_average) * 100
            else if comparison_amount > 0:
                100.0  # First spending in this category
            else:
                0.0
        
        annual_delta = (comparison_amount - monthly_average) * 12
        
        percentage_of_total = if total_expenses > 0:
            (comparison_amount / total_expenses) * 100
        else:
            0.0
        
        unique_months = get_unique_months(tag_transactions)
        months_of_data = count(unique_months)
        
        stat = TagExpenseStats {
            tag: tag,
            last_month_amount: comparison_amount,
            percentage_of_total: percentage_of_total,
            monthly_average: monthly_average,
            percentage_change: percentage_change,
            annual_delta: annual_delta,
            months_of_data: months_of_data,
        }
        
        if tag == "Other":
            other_stat = Some(stat)
        else:
            stats.push(stat)
    
    sort(stats, by last_month_amount descending)
    
    if other_stat is Some:
        stats.push(other_stat.unwrap())
    
    return stats
```

**Key Points:**

- Historical average **excludes** the comparison month
- This prevents circular comparison and allows trend detection
- If only one month exists, show InsufficientData state
- "Other" tag tracked separately for O(n) efficiency

**Tests:**

- Basic calculation correctness
- "Other" tag sorted to end
- Positive amounts excluded
- Empty input handling
- Historical average excludes comparison month
- Insufficient data detection (<2 complete months)
- Boundary percentages (5.5%) calculate correctly
- First month only (no historical baseline)

---

### Step 2: Create Card Rendering Module

**File:** `cards.rs` (NEW)

**Imports needed:**

```rust
use maud::{Markup, html};
use crate::dashboard::aggregation::TagExpenseStats;
use crate::html::{format_currency, LINK_STYLE};
use crate::endpoints;
```

**Constants:**

```rust
const DISPLAY_THRESHOLD: f64 = 5.5;  // Aligns with rounding - see below
```

**Main function:**

```rust
/// Renders the expense cards section with all tag statistics.
///
/// # Arguments
/// * `tag_stats` - Statistics for each tag to display
/// * `displayed_month_label` - Label for the month being shown (e.g., "December 2024")
pub(super) fn expense_cards_view(
    tag_stats: &[TagExpenseStats],
    displayed_month_label: &str,
) -> Markup
```

**Helper functions:**

```rust
fn determine_card_state(stat: &TagExpenseStats) -> CardState
fn expense_card(stat: &TagExpenseStats) -> Markup
fn card_bottom_content(stat: &TagExpenseStats, state: CardState) -> Markup
fn progress_bar(percentage: f64) -> Markup
fn empty_state_view() -> Markup
fn helper_card() -> Markup
fn format_percentage(value: f64) -> String
```

**Structure:**

```
expense_cards_view
├── If empty: return empty_state_view()
├── Section header with displayed_month_label
│   ├── "Expenses by Tag" (h3)
│   └── displayed_month_label (subtitle, right-aligned)
└── Grid of cards
    ├── For each tag: expense_card()
    └── If ≤2 tags: helper_card()

expense_card
├── Tag name (includes emoji if user added one)
├── Last month amount (large, use format_currency)
├── Percentage of total
├── Progress bar
└── card_bottom_content (based on state)

card_bottom_content (varies by state)
├── InsufficientData: "Building baseline..."
├── OnTrack: Average + "→ On track"
├── Overspending: Average + trend (red) + delta (red)
└── Saving: Average + trend (green) + delta (green) + 🎉

determine_card_state
├── Check months_of_data < 2 → InsufficientData
├── Check abs(percentage_change) < 5.5 → OnTrack
├── Check percentage_change >= 5.5 → Overspending
└── Else → Saving
```

**Key implementation notes:**

- Use `format_currency()` from `html.rs` for all monetary displays
- Use `LINK_STYLE` from `html.rs` for the "Manage Tags →" link
- Tag name displays exactly as stored - no emoji mapping needed
- Progress bar uses existing dark mode color scheme
- All cards shown regardless of size - no filtering by percentage
- Threshold is 5.5% to align with display rounding (see next section)

---

### Display Rounding and Threshold Alignment

**Critical Issue:** The UI rounds percentages to whole numbers, but state logic uses exact values.

**Example Problem:**

```
4.7% rounds to "5" but state is OnTrack
5.3% rounds to "5" but state is Overspending
Both display "5%" but show different states! ❌
```

**Solution:** Use 5.5% threshold so state aligns with displayed value:

```rust
const DISPLAY_THRESHOLD: f64 = 5.5;

fn determine_card_state(stat: &TagExpenseStats) -> CardState {
    if stat.months_of_data < 2 {
        return CardState::InsufficientData;
    }
    
    let abs_change = stat.percentage_change.abs();
    
    // Anything < 5.5% rounds to "5" or less → OnTrack
    // Anything >= 5.5% rounds to "6" or more → Overspending/Saving
    if abs_change < DISPLAY_THRESHOLD {
        CardState::OnTrack
    } else if stat.percentage_change >= DISPLAY_THRESHOLD {
        CardState::Overspending
    } else {
        CardState::Saving
    }
}
```

**Rule:** OnTrack = display shows "5%" or less, Overspending/Saving = display shows "6%" or more

**Tests:**

- 4.4% → "4" → OnTrack ✓
- 5.4% → "5" → OnTrack ✓
- 5.5% → "6" → Overspending ✓
- Consistency test: same displayed % = same state

---

### Step 3: Update Handlers

**File:** `handlers.rs`

**Modify:** `build_dashboard_data`

```rust
fn build_dashboard_data(
    excluded_tag_ids: &[DatabaseId],
    local_timezone_name: &str,
    connection: &Connection,
) -> Result<Option<DashboardData>, Error> {
    // ... existing code ...

    let today = OffsetDateTime::now_utc().to_offset(local_timezone).date();
    
    // Calculate last complete month
    let last_complete_month = (today.replace_day(1).unwrap() - Duration::days(1))
        .replace_day(1)
        .unwrap();
    
    // Format display label: "December 2024"
    let displayed_month_label = format_month_year_label(last_complete_month);
    
    let tag_stats = calculate_tag_expense_statistics(&transactions, last_complete_month);

    Ok(Some(DashboardData {
        tags_with_status,
        charts,
        tables,
        tag_stats,
        displayed_month_label,  // NEW
    }))
}
```

**New helper function:**

```rust
/// Formats a date as "MonthName YYYY" (e.g., "December 2024")
fn format_month_year_label(date: Date) -> String {
    use time::Month;
    let month_name = match date.month() {
        Month::January => "January",
        Month::February => "February",
        Month::March => "March",
        Month::April => "April",
        Month::May => "May",
        Month::June => "June",
        Month::July => "July",
        Month::August => "August",
        Month::September => "September",
        Month::October => "October",
        Month::November => "November",
        Month::December => "December",
    };
    format!("{} {}", month_name, date.year())
}
```

**Modify:** `DashboardData` struct

```rust
struct DashboardData {
    tags_with_status: Vec<TagWithExclusion>,
    charts: [DashboardChart; 3],
    tables: Vec<Markup>,
    tag_stats: Vec<TagExpenseStats>,
    displayed_month_label: String,  // NEW
}
```

**Modify:** `dashboard_view` and `dashboard_content_partial`

Pass `displayed_month_label` to `expense_cards_view`:

```rust
(expense_cards_view(&tag_stats, &data.displayed_month_label))
```

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

```rust
#[test]
fn calculate_tag_expense_statistics_basic() {
    // Test basic calculation with last complete month
}

#[test]
fn calculate_tag_expense_statistics_boundary_percentages() {
    // Test 5.5% boundary case
}

#[test]
fn calculate_tag_expense_statistics_excludes_comparison_month_from_average() {
    // Verify comparison month doesn't pollute average
}

#[test]
fn calculate_tag_expense_statistics_with_only_comparison_month() {
    // First month - insufficient data
}

#[test]
fn calculate_tag_expense_statistics_moves_other_to_end() {
    // "Other" tag sorting
}

#[test]
fn calculate_tag_expense_statistics_excludes_positive_amounts() {
    // Refunds/income excluded
}

#[test]
fn calculate_tag_expense_statistics_handles_empty_input() {
    // Empty vector handling
}
```

**cards.rs:**

```rust
#[test]
fn card_state_insufficient_data() {
    // < 2 months
}

#[test]
fn card_state_boundary_cases() {
    // 4.4%, 5.4%, 5.5%, 5.6% testing
}

#[test]
fn card_state_negative_boundary_cases() {
    // -5.5% boundary
}

#[test]
fn display_and_state_consistency() {
    // Parameterized test: same displayed % = same state
}

#[test]
fn renders_empty_state_when_no_tags() { }

#[test]
fn renders_helper_card_when_few_tags() { }

#[test]
fn progress_bar_has_minimum_width_for_small_percentages() { }
```

### Integration Tests

**handlers.rs:**

```rust
#[tokio::test]
async fn dashboard_shows_expense_cards_with_correct_month_label() {
    // Create transactions spanning multiple months
    // Verify cards show last complete month
    // Verify label shows correct month name
}

#[tokio::test]
async fn expense_cards_update_when_tags_excluded() {
    // Verify filtering still uses last complete month
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

- [ ] Each card has descriptive `aria-label` mentioning "last month"
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
- Only one complete month → InsufficientData state
- Zero division → guarded in percentage calculations
- Large deltas → show as calculated, no special handling

---

## Deployment Checklist

- [ ] All tests passing (`cargo test`)
- [ ] No clippy warnings (`cargo clippy`)
- [ ] Code formatted (`cargo fmt`)
- [ ] Manual testing in dev
- [ ] Verify month label updates correctly across month boundaries
- [ ] Test responsive behavior (mobile/tablet/desktop)
- [ ] Verify dark mode
- [ ] Check accessibility with screen reader
- [ ] Test with only 1 month of data (should show InsufficientData)
- [ ] Test across month transition (e.g., on Jan 1st)

---

## Configuration

### Constants

```rust
const DISPLAY_THRESHOLD: f64 = 5.5;  // % change for significant variance
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
- Click card → filter transactions by tag and month
- Hover tooltip with monthly breakdown
- Toggle between "last complete month" and "current month (projected)"
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

- Document the temporal logic (why last complete month)
- Explain historical average exclusion
- Reference design spec for UX decisions

### No User Documentation Needed

- Feature is self-explanatory (month label makes it clear)
- Follows existing dashboard patterns

---

## References

- Design Spec: `expenses-by-tag-design-spec.md`
- Existing Code: `handlers.rs`, `tables.rs`, `aggregation.rs`, `html.rs`
- Maud Docs: https://maud.lambda.xyz/
- Tailwind CSS: https://tailwindcss.com/

---

**Document Version:** 2.0
**Last Updated:** 2025-01-24
**Status:** Ready for Implementation
**Estimated Effort:** 5-7 hours
**Changes from v1.0:** Updated to use last complete month instead of current partial month
