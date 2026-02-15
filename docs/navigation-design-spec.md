# Navigation Design Spec

## Purpose

Define the current navigation design and implementation for Budgeteur.

## Current Design

### Information Architecture
- Primary destinations: Dashboard, Transactions, Accounts, Tags, Rules, Log out.
- Navigation is global and appears at the top of each authenticated page.

### Desktop Layout
- Top navigation bar with logo on the left and horizontal links on the right.
- Uses standard link styling with hover and active-state treatments.
- Active link is visually highlighted.

### Mobile/Tablet Layout
- Top header (logo/title) stays at the top.
- Primary navigation moves to a fixed bottom bar with a “More” popover.

### Visual Style
- Light theme uses white backgrounds and gray borders; dark theme uses dark backgrounds and muted text.
- Active state uses blue highlight.
- Uses Tailwind utility classes for spacing, typography, and colors.

### Accessibility
- Bottom nav uses `aria-current="page"` for the active item.
- “More” is implemented with `details/summary` for keyboard access without JS.
- Links use normal anchor semantics.

## Current Implementation

### Markup and Rendering
- Navigation is rendered by `NavBar` in `src/navigation.rs`.
- `NavBar::new(active_endpoint)` builds a fixed list of `Link` items.
- The active link is determined by comparing the current endpoint to the link URL.
- The logout link is never marked as active.

### Structure
- Top-level `<nav>` contains:
  - Brand/logo link to `/`.
  - Desktop links container with `ul > li > a` shown from `lg` and up.
- Bottom nav is rendered as a second `<nav>` and shown below `lg`.
- Link styles include active and inactive variants with `lg:` overrides for desktop.

### Interaction
- “More” uses native `details/summary` toggling (no JS).

### Call Sites
- `NavBar` is included in most page templates, e.g.
  - `src/dashboard/handlers.rs`
  - `src/transaction/transactions_page.rs` (via `transactions_view`)
  - `src/account/accounts_page.rs`
  - `src/tag/list.rs`
  - `src/rule/list.rs`
  - `src/transaction/view.rs`
  - `src/transaction/create_page.rs`
  - `src/transaction/edit_page.rs`
  - `src/account/create_page.rs`
  - `src/account/edit_page.rs`
  - `src/csv_import/import_page.rs`
  - `src/tag/create.rs`
  - `src/tag/edit.rs`
  - `src/rule/create.rs`
  - `src/rule/edit.rs`

### Goals
- Keep the current desktop navigation style and behavior.
- Replace the mobile/tablet hamburger menu with a bottom navigation bar.
- Style the bottom bar similar to “example 6” from Justinmind’s mobile navigation article.

### Breakpoints
- Bottom nav is shown for widths < 1024px.
- Existing top nav is shown at ≥ 1024px.

### Bottom Navigation (Tablet/Phone)
- Fixed bar at the bottom of the viewport.
- Always show four items (fixed set):
  - Dashboard, Transactions, Accounts, More.
- Additional items live behind “More”:
  - Tags, Rules, Log out.
- “More” opens a small popover above the bar via `details/summary`.
- Icons are a future enhancement; initial implementation can be text-only.
- Each item shows an icon + label.
- Active item gets a filled or highlighted pill/background treatment.
- Non-active items use neutral text and subtle hover states.
- Use a translucent or elevated surface (e.g., blurred backdrop or shadow) to separate from content.
- Preserve dark/light theme parity.

### Layout Adjustments
- Add safe-area spacing and bottom padding to main content so fixed bottom nav does not obscure page content.
- Remove or hide the hamburger button and collapsible top menu on tablet/phone sizes.
- Keep the top header (logo/title) visible on tablet/phone; only the nav links move to the bottom bar.

### Accessibility
- Add `aria-current="page"` on the active bottom nav item.
- Keep link text visible (not icon-only) for clarity and touch targets.
- Ensure “More” is keyboard accessible without JS (e.g., `details/summary` or focusable toggle).
- When a hidden item is the active page, highlight “More” in the bar and mark the hidden item as active in the popover.

### Implementation Notes
- Extend `NavBar` to render two nav variants:
  - Desktop top nav (existing structure).
  - Mobile/tablet bottom nav (new structure).
- Keep “More” behavior CSS-only; do not add JS.
- Switch responsive classes from `md` to `lg` to reserve the bottom nav for tablet/phone.
- Keep desktop styles unchanged to avoid regressions.
- Ensure `static/app.js` does not conflict with the new layout (hamburger may be removed or hidden on small screens).

## Implementation Decisions (Ad-Hoc)
- **Bottom nav styling:** Container uses `rounded-xl`; pill buttons use `rounded-lg` for a slightly tighter look.
- **Bottom nav width/gutters:** Wrapper uses `max-w-screen-xl` with a consistent `px-4` gutter to match page containers on landscape.
- **Z-index layering:** Bottom nav uses `z-40` so it sits above page content but below alerts.
- **ECharts tooltip stacking:** `.echarts-tooltip` forced to `z-index: 30` to prevent tooltips covering the bottom nav.
- **Body padding:** `pb-[calc(5rem+env(safe-area-inset-bottom))] lg:pb-0` on `body` to protect content from the fixed bottom nav.
- **Chart height responsiveness:** Dashboard chart containers use `min-h-[240px] sm:min-h-[300px] md:min-h-[340px] lg:min-h-[380px]` to keep axis labels visible on small/landscape screens.

### Future Enhancement (Optional)
- Consider progressively revealing hidden items inline on tablet widths, and hiding “More” when there is sufficient space.

---

**Document Version:** 1.0
**Last Updated:** 2026-02-15
**Status:** Implemented
**Changes from v0.x:** Formalized mobile bottom nav, responsive breakpoints, layout adjustments, and implementation decisions
