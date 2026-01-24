# Design Specification: Expenses by Tag Cards

## Overview

A card-based visualization showing expense breakdown by tag with trend indicators and annual spending impact.
Complements the existing dashboard charts and tables by providing an at-a-glance view of spending patterns across
categories.

**IMPORTANT: Cards display data for the LAST COMPLETE MONTH, not the current partial month.**

## Goals

- **Primary**: Help identify overspending categories quickly
- **Secondary**: Motivate behavioral change through annual delta projection
- **Tertiary**: Provide visual variety to break up table-heavy dashboard

## Design Principles

1. **Scannable**: Visual hierarchy allows absorbing information in <3 seconds per card
2. **Actionable**: Annual delta framing encourages spending adjustments
3. **Progressive**: Works well for first-time use and ongoing analysis
4. **Non-judgmental**: Neutral tone with celebration for savings, not shame for overspending
5. **Truthful**: Only show complete, accurate data - never partial or extrapolated

---

## Temporal Logic: Why Last Complete Month?

The cards compare **last complete month** against **historical average** (excluding that month).

### The Problem with Current Month

```
Today: January 15th
Current month: $500 spent
Historical avg: $800/month

Showing current month would say: "↓ -37% below usual"
But reality: User is on track to spend ~$1000 (overspending!)
```

Partial month data is **misleading** until the last day. Extrapolation is **unreliable** (big purchases, rent payments, irregular spending).

### The Solution: Last Complete Month

```
Today: January 15th (any day)
Last complete: December ($850)
Historical avg: $800/month (Jan-Nov average)

Card shows: "↑ +6% above usual" ✓
```

Benefits:

- Always accurate (complete data)
- Always comparable (full month vs full months)
- Simple to understand
- No edge cases or estimation errors

### User Mental Model

"Here's how I did last month compared to my usual pattern. I can use this to inform my behavior this month."

This is **actionable** ("I overspent last month, let me be careful") without being **false** (showing misleading partial data).

---

## Visual Design

### Section Layout

```
Dashboard Page
├── Navigation Bar
├── Charts Grid (2x2)
│   ├── Net Income Chart
│   ├── Net Balance Chart
│   ├── Summary Statistics Table
│   └── Monthly Summary Table
└── Expenses by Tag Section ⬅️ NEW
    ├── Section Header
    └── Card Grid
```

### Section Header

```
Expenses by Tag                                    December 2024
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
```

**Specifications:**

- Title: "Expenses by Tag" (text-xl, font-semibold)
- Subtitle: "December 2024" (text-sm, text-gray-600, right-aligned)
  - Format: `{MonthName} {Year}` (e.g., "December 2024", "March 2025")
  - Shows the actual month being displayed (last complete month)
- Divider: Subtle border or visual separator
- Margin: 8 units below Monthly Summary Table, 4 units above cards

---

## Card Design

### Standard Card (Normal Spending)

```
┌──────────────────────┐
│ 🍔 Food              │
│                      │
│ $450                 │
│ 35% of expenses      │
│ ▓▓▓▓▓▓▓░░░           │
│                      │
│ Avg: $420/month      │
│ ↑ +7% above usual    │
│ 💡 +$360/year        │
└──────────────────────┘
```

**Note:** Emojis (like 🍔) are entered directly in tag names by the user, not auto-generated. Tags without emojis display as plain text.

### Card Anatomy

| Element                 | Style               | Purpose                                                          |
| ----------------------- | ------------------- | ---------------------------------------------------------------- |
| **Tag Name**            | Large, semibold     | Quick category identification (includes any emoji from tag name) |
| **Month Amount**        | XX-large, bold      | Primary data point (last complete month)                         |
| **% of Total Expenses** | Small, muted        | Relative context                                                 |
| **Visual Bar**          | Horizontal progress | At-a-glance comparison                                           |
| **Monthly Average**     | Small, regular      | Historical baseline (excluding displayed month)                  |
| **Trend Indicator**     | Arrow + % + color   | Direction of change                                              |
| **Annual Impact**       | Bulb emoji + amount | Motivational projection                                          |

### Card States

#### 1. Overspending (≥5.5% above average)

```
┌──────────────────────┐
│ 🚗 Transport         │
│                      │
│ $950                 │
│ 28% of expenses      │
│ ▓▓▓▓▓▓░░░░           │
│                      │
│ Avg: $650/month      │
│ ↑ +46% above usual   │ ⬅️ Red text
│ 💡 +$3,600/year      │ ⬅️ Red text
└──────────────────────┘
```

#### 2. Saving (≤-5.5% below average)

```
┌──────────────────────┐
│ 🎬 Entertainment     │
│                      │
│ $120                 │
│ 8% of expenses       │
│ ▓▓░░░░░░░░           │
│                      │
│ Avg: $200/month      │
│ ↓ -40% below usual   │ ⬅️ Green text
│ 💡 -$960/year 🎉     │ ⬅️ Green text + celebration
└──────────────────────┘
```

#### 3. On Track (<5.5% variance)

```
┌──────────────────────┐
│ ⚡ Utilities         │
│                      │
│ $215                 │
│ 12% of expenses      │
│ ▓▓▓░░░░░░░           │
│                      │
│ Avg: $220/month      │
│ ↑ On track           │ ⬅️ Gray text, no annual delta
└──────────────────────┘
```

#### 4. Insufficient Data (<2 complete months)

```
┌──────────────────────┐
│ 🍔 Food              │
│                      │
│ $450                 │
│ 35% of expenses      │
│ ▓▓▓▓▓▓▓░░░           │
│                      │
│ Building baseline... │ ⬅️ Blue/info color
└──────────────────────┘
```

**Note:** Requires at least 2 complete months: one for display, one for comparison.

#### 5. No Tags / Empty State

```
┌──────────────────────┐
│ 💡 Get Started       │
│                      │
│ Add tags to see      │
│ detailed spending    │
│ breakdown!           │
│                      │
│ Tags help you        │
│ understand where     │
│ your money goes.     │
│                      │
│ [Manage Tags →]      │ ⬅️ Link to tag management
└──────────────────────┘
```

#### 6. Helper Card (1-2 tags only)

```
┌──────────────────────┐
│ 💡 Tip               │
│                      │
│ Add more tags to     │
│ see detailed         │
│ spending breakdown!  │
│                      │
│ Keep tags broad      │
│ (aim for ~10 tags).  │
│                      │
│ [Manage Tags →]      │
└──────────────────────┘
```

---

## Responsive Behavior

### Breakpoints

```
Mobile:    1 column  (< 640px)   ┌───┐
                                 │ A │
                                 ├───┤
                                 │ B │
                                 └───┘

Small:     2 columns (≥ 640px)   ┌───┬───┐
                                 │ A │ B │
                                 ├───┼───┤
                                 │ C │ D │
                                 └───┴───┘

Medium:    3 columns (≥ 768px)   ┌───┬───┬───┐
                                 │ A │ B │ C │
                                 └───┴───┴───┘

Large:     4 columns (≥ 1024px)  ┌───┬───┬───┬───┐
                                 │ A │ B │ C │ D │
                                 └───┴───┴───┴───┘
```

**Tailwind classes:** `grid-cols-1 sm:grid-cols-2 md:grid-cols-3 lg:grid-cols-4`

### Card Dimensions

- **Min width:** 180px (prevents over-stretching on wide screens)
- **Max width:** None (fills grid cell)
- **Min height:** 200px (consistent card height)
- **Padding:** 4 units internal spacing
- **Gap:** 4 units between cards

---

## Interaction & Accessibility

### Hover States

- **Subtle elevation:** Shadow increases slightly (`shadow-md` → `hover:shadow-lg`)
- **No pointer cursor:** Cards are informational, not clickable (yet)
- **Future:** Could add drill-down to filtered transaction list

### Accessibility

- **Semantic HTML:** Each card is a `<div>` with appropriate ARIA labels
- **Screen reader text:**
  ```
  aria-label="Food expenses: $450 last month, up 7% from usual $420 average, 
             resulting in $360 more spending per year"
  ```
- **Color not sole indicator:** Arrows (↑↓→) supplement color for colorblind users
- **Focus indicators:** Visible outline for keyboard navigation (if interactive features added)

### Keyboard Navigation

- Not applicable (no interactive elements currently)
- Future: Tab through cards, Enter to drill down

---

## Data & Logic

### Temporal Calculation

```rust
// Get last complete month
let today = get_current_date();
let last_complete_month = (today.replace_day(1) - 1.day).replace_day(1);

// Example: 
// Today: 2025-01-15 → last_complete_month: 2024-12-01
// Today: 2025-01-01 → last_complete_month: 2024-12-01
// Today: 2025-01-31 → last_complete_month: 2024-12-01
```

### Calculations

#### Monthly Average (Historical Baseline)

```
# Exclude the month being displayed from average calculation
historical_months = all_months.excluding(last_complete_month)
average = sum(historical_months) / count(historical_months)

# Special case: if only one month of data exists (no historical baseline)
if count(historical_months) == 0:
    insufficient_data = true
```

**Why exclude displayed month:**

- Prevents the comparison from being circular
- "How does December compare to my typical month?" means "typical" shouldn't include December
- Allows detecting trends: if December is always high, average stays accurate

#### Percentage Change

```
if average > 0:
    change_pct = ((last_month_amount - average) / average) * 100
else if last_month_amount > 0:
    change_pct = 100  # First month with spending
else:
    change_pct = 0
```

#### Annual Delta

```
annual_delta = (last_month_amount - average) * 12
```

### Display Rules

| Condition                     | Display                                       |
| ----------------------------- | --------------------------------------------- |
| `historical_months_count < 1` | "Building baseline..."                        |
| `abs(change_pct) < 5.5%`      | "On track" (no annual delta)                  |
| `change_pct >= 5.5%`          | "↑ +X% above usual" + red annual delta        |
| `change_pct <= -5.5%`         | "↓ -X% below usual" + green annual delta + 🎉 |

**Note:** Threshold is 5.5% (not 5.0%) to align with display rounding - see Technical Spec for details.

### Number Formatting

- **Amounts:** Use existing `format_currency()` function from `html.rs`
  - Outputs: `$1,234.00` (comma separator, 2 decimals)
- **Percentages:** `+7%` (no decimals, include sign)
- **Annual delta:** `+$360/year` (include sign, "/year" suffix)
- **Month display:** `"December 2024"` format (full month name + year)

### Sorting

Default order: **Amount descending** (largest expenses first)

Rationale: Focus on biggest spending categories first.

Future enhancement: Could add sort options if needed.

---

## Edge Cases

### 1. Only One Complete Month

Show the tag card + helper card + "Building baseline..." state

**Rationale:** Need at least 2 months (one to display, one for comparison)

### 2. Many Tags (>15)

All tags shown - no hiding or "show more" button. User should be encouraged to consolidate tags.

### 3. Small Tags (<2% of expenses)

**Always show** - no threshold for hiding. Better to see all categories.

### 4. Exact Zero Change

```
│ Avg: $500/month      │
│ → On track           │
```

### 5. Very Large Delta

```
│ 💡 +$12,360/year     │
```

No special handling - show as calculated.

### 6. Negative Expenses (Refunds)

If a tag has net positive amount (refunds > expenses) in the displayed month, exclude from this section.

**Decision:** Exclude tags with positive net amounts (they're not expenses).

### 7. "Other" Tag Placement

Always sort "Other" (untagged transactions) to the **end** of the list, regardless of amount.

### 8. First Day of Month

On January 1st:

- Last complete month: December
- Works perfectly - no special case needed

### 9. Month Transitions

The displayed month updates automatically:

- Jan 1-31: Shows "December 2024"
- Feb 1-28: Shows "January 2025"
- Smooth transition, always accurate

---

## Visual Design Tokens

### Colors (Use existing classes from `html.rs`)

- **Overspending:** `text-red-600 dark:text-red-400`
- **Saving:** `text-green-600 dark:text-green-400`
- **On track:** `text-gray-600 dark:text-gray-400`
- **Insufficient data:** `text-blue-600 dark:text-blue-400`
- **Card background:** `bg-white dark:bg-gray-800`
- **Card border:** `border border-gray-200 dark:border-gray-700`

### Typography

- **Tag name:** `text-lg font-semibold`
- **Current amount:** `text-3xl font-bold`
- **Percentage:** `text-sm text-gray-600 dark:text-gray-400`
- **Average:** `text-sm`
- **Trend:** `text-sm font-medium`
- **Annual delta:** `text-sm font-semibold`

### Spacing

- **Card padding:** `p-4`
- **Grid gap:** `gap-4`
- **Internal spacing:** `space-y-2` between elements
- **Section margin:** `mt-8 mb-8`

### Shadows & Borders

- **Card elevation:** `shadow-md hover:shadow-lg`
- **Rounded corners:** `rounded-lg`
- **Transition:** `transition-shadow` for smooth hover effect

---

## Implementation Notes

### Emojis in Tags

- Users enter emojis directly in tag names (e.g., "🍔 Food", "Transport")
- No automatic emoji mapping or picker
- Tags display exactly as entered
- Emojis are optional - plain text tags work fine

### Tag Organization Philosophy

- Encourage **broad categories** (~10 tags total)
- Show all tags regardless of size
- "Other" tag for untagged transactions appears last

### Integration with Existing Code

- Use `format_currency()` from `html.rs` for all monetary values
- Follow existing Tailwind class patterns
- Match existing table/chart styling for consistency
- Use existing color scheme (blue accent, dark mode support)

---

## Future Enhancements

Could consider adding:

- Click to filter transactions by tag
- Tooltip with monthly breakdown on hover
- Sort options (by amount, by %, by name)
- Sparkline showing last few months
- Toggle to show "current month (projected)" vs "last complete month"

---

## Open Questions

None - design is complete and ready for implementation.

---

**Document Version:** 2.0
**Last Updated:** 2025-01-24
**Status:** Ready for Implementation
**Changes from v1.0:** Updated temporal logic to use last complete month instead of current partial month
