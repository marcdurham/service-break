# Service Break — Frontend Style Guide

Reference for adding or modifying UI in the Yew frontend. Covers design tokens, layout rules, and component patterns. The full visual spec is in `styles-audit.md`.

## Design Tokens (CSS Variables)

All colours live in `:root` in `assets/styles.css`. Use them — never hard-code hex values for UI elements.

| Token | Value | Where |
|-------|-------|-------|
| `--bg` | `#f6efe4` | Page background |
| `--ink` | `#2c211a` | Headings, primary text |
| `--ink-soft` | `#4c4034` | Body text on white cards |
| `--muted` | `#6b5a48` | Secondary text, labels, subtitles (WCAG AA 4.5:1) |
| `--muted2` | `#9a8a78` | Tertiary text |
| `--faint` | `#b0a08c` | Very subtle hints |
| `--accent` | `#934228` | Primary CTA, links, active states (WCAG AA) |
| `--accent-dark` | `#7a3620` | Hover/pressed accent |
| `--border` | `#e5dac8` | Dividers, input borders |
| `--green` | `#6f8256` | Success, cleanliness indicators |
| `--danger` | (inline `#b83a3a`) | Delete/error actions |

**Fonts:** DM Sans (body/UI), Instrument Serif italic (display/headings). Icons: Material Symbols Rounded, filled, weight 500.

## Layout Rules

- **Mobile-first.** Everything is designed for narrow screens first; the `@media (min-width: 768px)` block converts the bottom tab bar into a left nav rail and repositions floating chrome.
- **Page padding:** `.screen-padding` → `padding-inline: 20px`. Most pages use `.screen` which already includes safe-area insets.
- **Max content width:** 680px on detail overlays (`max-width: 680px; margin: 0 auto`). Other pages fill the screen width minus padding.
- **Horizontal overflow guard.** Any flex row that may contain multiple items (buttons, chips, tags) must allow wrapping or horizontal scrolling — never let content push past the viewport edge. Use `overflow-x: auto` with hidden scrollbar for button rows; use `flex-wrap: wrap` for tag/chip rows.

## Spacing System

Use utility classes from `styles.css`, not inline styles:

| Class | Value |
|-------|-------|
| `.section-gap` | `margin-top: 24px` (between major sections) |
| `.field-gap` | `margin-top: 16px` (between form fields) |
| `.card-gap` | `gap: 10px` (between cards) |
| `.mt-sm` / `.mt-md` | `12px` / `18px` top margin |
| `.mb-sm` / `.mb-md` | `8px` / `14px` bottom margin |
| `.pb-24` | `padding-bottom: 24px` (bottom safe area) |
| `.pt-18` | `padding-top: 18px` |

**Rule:** If you find yourself writing `style="margin-bottom: X"` in a component, check the table above first. Only add an inline style when no utility class matches.

## Button Sizes (Normalized)

| Class | Height / Radius | Use For |
|-------|-----------------|---------|
| `.submit-btn` | 50px tall, `border-radius: 16px`, full-width accent CTA | Primary actions (save, submit, invite) |
| `.alt-auth-btn` | 46px min-height, `border-radius: 14px`, outlined white | Secondary actions (Google sign-in, back buttons) |
| `.directions-btn` | Full-width accent, `border-radius: 15px` | Navigation CTA in detail view |
| `.rate-btn` | Outlined, `border-radius: 15px`, icon + label | Secondary actions in action-row |

**Rule:** Never invent a new button class. Reuse one of the four above. If a button needs to be wider/narrower, adjust padding — not the class.

## Button Padding (Consistent)

All buttons in the same row must use identical `padding` values so text-length differences don't create optical inconsistency:

| Context | Horizontal Padding |
|---------|-------------------|
| `.directions-btn` inside `.action-row` | `15px 14px` |
| `.rate-btn` inside `.action-row` | `15px 14px` |
| Base `.rate-btn` (outside action-row) | `15px 14px` |

**Rule:** When adding a button to an existing row, match the row's horizontal padding exactly. Verify by checking that adjacent buttons with different text lengths (e.g., "Map" vs "Delete") have visually equal internal spacing.

## Typography Scale

| Context | Size / Weight | Class or element |
|---------|---------------|------------------|
| Screen title | 28px Instrument Serif italic | `.screen-title` |
| Sheet/dialog title | 26px Instrument Serif | `.sheet-title`, `.invite-h1` |
| Section title | 15–17px DM Sans semibold | `.section-title`, `.reviews-title` |
| Body text | 14–15px DM Sans regular | default |
| Field label (form) | 12px uppercase, `--muted`, letter-spacing .7px | `.field-label` |
| Subtitle / helper | 13–14px `--muted` | `.screen-sub`, `.auth-note` |

**Rule:** All button text is `font-size: 15–16px; font-weight: 700`. Never use a different size for interactive text.

## Icon Sizes

| Class | Size |
|-------|------|
| `.mi-sm` | 16px |
| `.mi-md` | 20px (default) |
| `.mi-lg` | 28px |

Icons are `<span class="mi">{"icon_name"}</span>`. Material Symbols Rounded filled set.

## Avatar Scale

| Class | Size | Use For |
|-------|------|---------|
| `.avatar-sm` | 32×32 | Inline badges |
| `.avatar-md` | 40×40 | User rows, edit forms |
| `.avatar-lg` | 56×56 | Detail hero, list cards |
| `.avatar-xl` | 72×72 | Account page header |

## Contrast Requirements (WCAG AA)

- Normal text (< 18px): **4.5:1** minimum against background.
- Large text (≥ 18px bold or ≥ 24px regular): **3:1**.
- `--muted` on `--bg`: 5.78:1 ✅ — safe for all normal text.
- `--accent` white-on-accent: 5.53:1 ✅ — safe for button text.
- Never introduce a new colour pair without checking the ratio. When in doubt, use `--ink` on `--bg` or `--muted` on `--bg`.

## Component Patterns

### Full-width screen (list / add / saved / invite / admin)

```html
<div class="screen sb-scroll">
  <div class="screen-title">{"Title"}</div>
  <div class="screen-sub mb-md">{"Subtitle"}</div>
  <!-- content -->
</div>
```

### Overlay sheet (detail / edit / filters)

```html
<div class="detail-overlay">
  <div class="detail-scroll sb-scroll">
    <div class="hero" style={format!("background:{}", color)}>...</div>
    <div class="detail-body">
      <!-- content -->
    </div>
  </div>
</div>
```

### Form fields

```html
<div class="field-label">{"Label text"}</div>
<input class="input" ... />
<!-- or for icon-prefix inputs: -->
<div class="input-row">
  <span class="mi">{"icon"}</span>
  <input ... />
</div>
```

### Button rows (action-row, composer-actions)

Always allow horizontal overflow on narrow screens:
```css
/* For button rows with fixed-count items — allows wrapping when no horizontal room */
.row { display: flex; gap: 9px; overflow-x: auto; scrollbar-width: none; flex-wrap: wrap; }
.row::-webkit-scrollbar { display: none; }
/* For tag/chip rows that should wrap */
.tag-row { display: flex; gap: 7px; flex-wrap: wrap; }
```

**Rule:** Button rows must use `flex-wrap: wrap` so buttons can float down to the next line when there is no horizontal room. Never force a single row with `nowrap` — it creates horizontal scroll on narrow screens.

## What Not To Do

- Don't add new CSS variables — extend the existing palette.
- Don't use inline `style="..."` for layout (margins, padding, flex). Only acceptable for dynamic values like colours from data or computed sizes.
- Don't create new button classes when one of `.submit-btn`, `.alt-auth-btn`, `.directions-btn`, `.rate-btn` fits.
- Don't break mobile-first — test at 320px width before committing layout changes.
- Don't change `--muted`, `--accent`, or other tokens without re-checking contrast ratios against both `--bg` and white card backgrounds.
