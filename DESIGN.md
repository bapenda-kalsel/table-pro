# Design System: TablePro

## 0. Brand Identity & Logo System (`/brandkit`)
- **Core Metaphor:** *Precision Relational Matrix (`T` + `P` Monogram)*.
- **Symbol Construction:** Built on an 8×8 modular grid inside a machined squircle (`#141418` fill, `#3F3F46` perimeter frame).
  1. **Top Schema Header Bar (`#F4F4F5`):** Forms the horizontal crossbar of `T` and represents a database table header with a recessed key slot (`#0E0E11`) and status node (`#0D9488`).
  2. **Primary Key Spine (`#F4F4F5`):** Forms the vertical stem shared by `T` and `P`, representing the primary key column.
  3. **Active Staged Cell (`#2DD4BF`):** Forms the upper loop of `P` in Calibrated Verdigris, representing an inspected/staged cell in an atomic transaction.
  4. **Relational Matrix Node (`#1C1C24` + `#2DD4BF` border):** Completes the 2×2 tabular grid beneath the schema header.
- **Brand Assets:**
  - Master Vector Mark: `static/logo.svg`, `docs/logo.svg`, and Svelte component `src/lib/components/chrome/Logo.svelte`
  - 3×3 Brand Identity Presentation Board: `docs/brandkit.svg` & `docs/brandkit.png`
  - Native App Icons: `src-tauri/icons/*` (`.icns`, `.ico`, `.png`), `static/favicon.png`, `docs/icon.png`

## 1. Visual Theme & Atmosphere
TablePro operates across two calibrated surfaces governed by a unified architectural identity: a high-density native database workbench (`VISUAL_DENSITY: 8`, `DESIGN_VARIANCE: 6`, `MOTION_INTENSITY: 6`) and an asymmetric editorial-technical product showcase (`DESIGN_VARIANCE: 8`, `MOTION_INTENSITY: 6`, `VISUAL_DENSITY: 4`).

The atmosphere is clinical, tactile, and machined — like a precision-milled Swiss engineering instrument paired with an architecture studio's light table. Surfaces are built from disciplined Zinc neutrals (`#0E0E11` to `#18181B` in dark mode; `#F4F4F5` to `#FFFFFF` in light mode), separated by 1px structural hairlines rather than heavy drop shadows. A single calibrated Teal-Cyan accent (`#0D9488` in light mode / `#2DD4BF` in dark mode, saturation < 75%) anchors all primary actions, active tab indicators, and focus rings without ever resorting to neon purple or multi-color rainbow toolbars.

## 2. Color Palette & Roles

### Dark Theme (Default Workbench & Showcase)
- **Obsidian Canvas** (`#0E0E11`) — Primary application background and root viewport surface (never pure `#000000`)
- **Graphite Panel** (`#141418`) — Secondary chrome surface for title bar, tab strip, status bar, and modal shells
- **Recessed Well** (`#111114`) — Subtle recessed surface for sidebars, SQL log console, and code blocks
- **Machined Surface** (`#1C1C22`) — Elevated container fill for active tabs, dropdown menus, and command palettes
- **Tactile Hover** (`#26262E`) — Interactive hover state for rows, tree nodes, and toolbar buttons
- **Hairline Edge** (`#27272A`) — Primary 1px structural divider across panels, cells, and toolbars
- **Structural Frame** (`#3F3F46`) — Emphasized border for active inputs, modal perimeters, and ER diagram nodes
- **Chalk Ink** (`#F4F4F5`) — Primary high-contrast text for headers, table names, and SQL identifiers
- **Alloy Text** (`#D4D4D8`) — Secondary body copy, data grid cell contents, and menu labels
- **Muted Steel** (`#A1A1AA`) — Column headers, field labels, and supporting descriptions
- **Etched Slate** (`#71717A`) — Metadata, row counts, keyboard shortcuts, and `NULL` placeholders
- **Calibrated Verdigris** (`#2DD4BF`) — Singular primary accent for CTAs, active tab indicators, focus rings, and selected states (HSL 172°, 66% saturation)
- **Deep Verdigris** (`#0D9488`) — Solid button fill and pressed accent state

### Light Theme (Daylight Workbench)
- **Alabaster Canvas** (`#F4F4F5`) — Primary application background surface
- **Pure Surface** (`#FFFFFF`) — Panel, modal, and data grid container fill
- **Paper Well** (`#FAFAFA`) — Sidebar, SQL log console, and alternating table row surface
- **Stone Elevated** (`#EFEFF1`) — Elevated interactive surface and badge fill
- **Pebble Hover** (`#E4E4E7`) — Row and button hover surface
- **Whisper Border** (`#E4E4E7`) — Primary 1px structural hairline
- **Alloy Border** (`#D4D4D8`) — Input and card perimeter border
- **Charcoal Ink** (`#18181B`) — Primary text, Zinc-900 depth (never pure `#000000`)
- **Graphite Text** (`#27272A`) — Data grid cell text and secondary labels
- **Slate Muted** (`#52525B`) — Descriptions, inactive tabs, and toolbar icons
- **Silver Faint** (`#71717A`) — Timestamps, row indices, and `NULL` markers
- **Calibrated Teal** (`#0D9488`) — Singular primary accent for CTAs, active indicators, and focus rings
- **Deep Teal** (`#0F766E`) — Primary button hover/active state

### Semantic Data & Transaction Tokens (Dual-Theme Calibrated)
Used strictly for real database state (staged INSERT/UPDATE/DELETE diffs, environment guard badges, and query execution status) — never as decorative icon coloring:
- **Staged Insert / Healthy** — Surface `rgba(16, 185, 129, 0.12)`, Border `rgba(16, 185, 129, 0.32)`, Text `#10B981` (Dark) / `#047857` (Light)
- **Staged Update / Caution** — Surface `rgba(245, 158, 11, 0.12)`, Border `rgba(245, 158, 11, 0.32)`, Text `#F59E0B` (Dark) / `#B45309` (Light)
- **Staged Delete / Production Guard** — Surface `rgba(244, 63, 94, 0.12)`, Border `rgba(244, 63, 94, 0.32)`, Text `#F43F5E` (Dark) / `#BE123C` (Light)

## 3. Typography Rules
- **Display & UI Sans:** `Geist`, `-apple-system`, `BlinkMacSystemFont`, `sans-serif` — Track-tight (`-0.025em` to `-0.035em`) on headlines, controlled scale, weight-driven hierarchy (`500` Medium and `600` SemiBold over brute-force size).
- **Body Copy:** `Geist` — Relaxed leading (`1.6`), maximum `65ch` line width on marketing and documentation surfaces, `12px` (`0.75rem`) to `13px` (`0.8125rem`) crisp density inside the workbench.
- **Monospace & Data:** `Geist Mono`, `JetBrains Mono`, `monospace` — Mandatory for all SQL editors, data grid cells, primary/foreign key badges, execution durations, byte sizes, row counts, and schema types. Always enforce `font-variant-numeric: tabular-nums` so numeric columns align vertically to the pixel.
- **Typographic Scale:**
  - Marketing Hero Display: `clamp(2.5rem, 4.8vw, 4rem)`, `line-height: 1.06`, `letter-spacing: -0.035em`
  - Section Header: `clamp(1.5rem, 2.8vw, 2.25rem)`, `line-height: 1.15`, `letter-spacing: -0.025em`
  - Workbench Panel Title: `0.8125rem` (`13px`), `font-weight: 600`, `letter-spacing: -0.01em`
  - Data Grid & SQL Cell: `0.6875rem` (`11px`), `font-family: Geist Mono`, `tabular-nums`
  - Micro Metadata / Eyebrow: `0.625rem` (`10px`), `letter-spacing: 0.08em`, uppercase
- **Banned Fonts:** `Inter`, `Roboto`, `Open Sans`, `Arial`, and all serif fonts (`Times New Roman`, `Georgia`, `Garamond`, `Fraunces`, `Instrument Serif`). Serif is strictly forbidden in database dashboards and software UIs.

## 4. Component Stylings
* **Hero Section (Product Showcase):** Left-aligned asymmetric split (`1.15fr 0.85fr` on desktop) inside a `min-h-[100dvh]` viewport container. Headline embeds inline visual pills (rounded type-height badges housing engine glyphs and live transaction telemetry) directly between words as visual punctuation. Maximum one primary CTA button in the hero; no secondary "Learn more" clutter, no scroll arrows, and no overlapping text layers.
* **Buttons:** Flat architectural surfaces with a 1px structural border (`border-edge`) or solid Calibrated Verdigris fill (`#0D9488`). Tactile `-1px` vertical translation (`active:translate-y-px`) on press. Focus states use a crisp 2px accent ring (`focus-visible:ring-2`). Never use neon outer glows.
* **Cards & Panels:** Inside the high-density workbench (`VISUAL_DENSITY: 8`), avoid floating cards with drop shadows; separate functional zones with 1px `border-edge` hairlines and `border-t` dividers. When elevation is required (modals, command palette, floating staged-changes bar, showcase bento tiles), use a `12px` (`0.75rem`) outer radius, `8px` (`0.5rem`) inner element radius, and a hue-tinted ambient shadow (`0 20px 50px rgba(8, 10, 14, 0.45)`).
* **Inputs & Forms:** Label positioned above the input (`10px` uppercase `Geist`, `font-weight: 600`, `text-muted`), optional helper text below, and inline error reporting below in semantic rose. Standard `0.375rem` (`6px`) vertical gap between label and control. No floating labels or placeholder-as-label patterns.
* **Loading States:** Layout-matched skeletal shimmer bars (`animate-pulse` with staggered widths matching table columns, stat strips, or schema trees). Generic circular spinners (`animate-spin` border rings) are banned in content panels.
* **Empty States:** Composed architectural empty states featuring a framed geometric icon badge, a concise primary heading, a 1-line actionable instruction (with keyboard shortcut `<kbd>` pills where applicable), and a direct action button when relevant.
* **Error States:** Inline left-accented callout blocks (`border-l-2`, semantic error surface and text) displaying the exact driver or SQL error message in selectable `Geist Mono` without `window.alert()` popups.

## 5. Layout Principles
- **Grid-First Architecture:** Use CSS Grid with explicit fractional tracks (`grid-template-columns`) for all multi-column layouts. Never use complex `calc()` flexbox percentage math.
- **Asymmetric Showcase & Bento Rhythm:** On the landing page, ban the generic "3 equal cards horizontally" row. Present the three database engines and platform downloads through asymmetric 12-column bento compositions (`7/5` and `5/7` splits, or `4/4/4` with a full-width architectural header bar and varied internal density) and paired split-screen code inspection blocks.
- **Workbench Spatial Zones:** Fixed `40px` (`h-10`) top title bar, `240px` (`w-60`) left schema navigator, `36px` (`h-9`) tab strip, fluid central canvas, and collapsible `176px` (`h-44`) bottom SQL console. Every element occupies its own spatial zone with zero overlapping text.
- **Responsive Collapse (< 768px):** All multi-column showcase grids, bento cells, and split sections collapse strictly to a single column (`grid-template-columns: 1fr`) below `768px` with zero horizontal overflow. Inline headline badges wrap cleanly, and all interactive controls maintain a minimum `44px` touch target on mobile viewports.
- **Viewport Stability:** Full-height sections use `min-h-[100dvh]` (or `h-[100dvh]` for the locked desktop app shell) — never `100vh`.

## 6. Motion & Interaction
- **Spring Physics:** Interactive transitions follow a weighted spring curve (`cubic-bezier(0.16, 1, 0.3, 1)`, equivalent to `stiffness: 100, damping: 20`). No linear easing.
- **Perpetual Micro-Interactions:** Active telemetry elements feature restrained, meaningful infinite loops — a subtle status pulse on active production guards and pending staged-change indicators, and a smooth horizontal progress shimmer along the top edge of the data grid during background re-fetches.
- **Staggered Orchestration:** Showcase sections and modal lists reveal with cascaded entry delays (`60ms` stagger per item) gated behind `@media (prefers-reduced-motion: no-preference)`.
- **GPU Discipline:** Animate exclusively via `transform` and `opacity`. Never animate `width`, `height`, `top`, or `left`. Apply `backdrop-filter` only to fixed/sticky chrome layers and modal backdrops.

## 7. Anti-Patterns (Banned)
- **NEVER** use emojis anywhere in the UI, table headers, primary key indicators, or status messages (use crisp `lucide-svelte` vector glyphs such as `KeyRound` instead of `🔑`, and `AlertTriangle` instead of `⚠`).
- **NEVER** use em-dashes (`—`) or en-dashes (`–`) in UI copy, tooltips, empty states, or marketing text. Use a standard hyphen (`-`), colon, or period.
- **NEVER** use `Inter`, `Roboto`, or any serif font.
- **NEVER** use pure black (`#000000`) for backgrounds or shadows.
- **NEVER** use the "AI Purple/Blue Neon" aesthetic (`from-sky-500 to-indigo-500` gradients, violet/purple toolbar buttons, or neon outer glows).
- **NEVER** colorize toolbar icons like a rainbow (amber search, violet ERD, cyan dashboard, violet audit, emerald wrench). Keep chrome icons in unified neutral `text-muted` that shift to `text-fg` or singular `text-accent` on hover/active.
- **NEVER** hardcode dark-only Tailwind utilities (`bg-emerald-950/40`, `text-amber-200`, `bg-rose-950/90`) that break contrast in Light Mode. Always use semantic theme variables.
- **NEVER** use generic circular spinners (`animate-spin` border rings) for panel loading states; use layout-matched skeletal shimmer rows.
- **NEVER** use 3-column equal card rows (`repeat(3, 1fr)` identical cards) on marketing or showcase pages.
- **NEVER** use AI copywriting clichés ("Elevate", "Seamless", "Unleash", "Next-Gen") or scroll-cue filler ("Scroll to explore", bouncing chevrons).
