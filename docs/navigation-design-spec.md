# Navigation Design Spec

## Purpose

Define the current navigation design and implementation for Budgeteur, then outline the proposed mobile/tablet bottom navigation update.

## Current Design

### Information Architecture
- Primary destinations: Dashboard, Transactions, Accounts, Tags, Rules, Log out.
- Navigation is global and appears at the top of each authenticated page.

### Desktop Layout
- Top navigation bar with logo on the left and horizontal links on the right.
- Uses standard link styling with hover and active-state treatments.
- Active link is visually highlighted.

### Mobile Layout
- Top navigation bar includes a hamburger button.
- Links are hidden by default and expand into a vertical list when the hamburger is toggled.

### Visual Style
- Light theme uses white backgrounds and gray borders; dark theme uses dark backgrounds and muted text.
- Active state uses blue highlight.
- Uses Tailwind utility classes for spacing, typography, and colors.

### Accessibility
- Hamburger button has `aria-controls` and `aria-expanded` and toggles the menu’s visibility.
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
  - Hamburger button (shown at small breakpoints).
  - Links container with `ul > li > a`.
- The links container is hidden at small breakpoints and shown from `md` up via `md:block`.
- Link styles include active and inactive variants with `md:` overrides.

### Interaction
- Mobile expansion is driven by a small JS toggle in `static/app.js` that:
  - Toggles the `hidden` class on the menu.
  - Updates `aria-expanded` on the button.

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

## Proposed Changes

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

### Future Enhancement (Optional)
- Consider progressively revealing hidden items inline on tablet widths, and hiding “More” when there is sufficient space.
