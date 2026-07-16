# Service Break — Style Audit & Contrast Analysis

Generated from reviewing all frontend components against `styles.css` and the design mockup.

---

## 1. Design Tokens (CSS Variables)

| Token | Value | Usage |
|-------|-------|-------|
| `--bg` | `#f6efe4` | Page background (warm cream) |
| `--ink` | `#17110d` | Primary text, headings (near-black warm brown) |
| `--muted` | `#6b5a48` | Secondary text, labels, subtitles |
| `--border` | `#cfc2b3` | Dividers, input borders, card separators |
| `--accent` | `#934228` | Primary CTA buttons, links, active states |
| `--accent-light` | `#e9d5c4` | Subtle highlights, hover backgrounds |
| `--success` | `#6f8256` | Success messages, parking-easy tags |
| `--danger` | `#a54e2c` | Error text, delete buttons, warnings |
| `--card-bg` | `#fffaf3` | Card/inset backgrounds (slightly warmer than page) |

**Fonts:**
- Body / UI: DM Sans (regular 400, medium 500, semibold 600)
- Display / Hero: Instrument Serif italic
- Icons: Material Symbols Rounded (filled, wght 500)

---

## 2. Page-by-Page Style Inventory

### Map Tab (`map_view.rs`)
- **Background:** `--bg` (#f6efe4), full-screen map canvas underneath
- **Top bar:** White pill (`background: #fff`, `box-shadow`), ~44px tall, `border-radius: 22px`, padding 8px 10px
  - Search input: DM Sans 15px, placeholder `--muted`, no visible border
  - Filter button: circular 36×36px, `background: #f6efe4`, icon 20px
- **Filter chips row:** Horizontal scroll, 8px gap between chips
  - Chip: DM Sans medium 13px, pill shape (`border-radius: 999px`), padding 7px 14px
  - Active chip: `background: #fff`, `color: var(--ink)`, icon filled
  - Inactive chip: transparent bg, `color: var(--muted)`
- **Place pins:** Colored by cleanliness (5-star green → 1-star red gradient)
- **Bottom sheet handle:** 36×4px rounded bar, `background: #cfc2b3`, margin-top 10px
- **Sheet header:** Title DM Sans semibold 18px, subtitle DM Sans regular 13px / `--muted`
- **Place row in sheet:** Avatar circle 44×44px (place-type colored), name DM Sans semibold 16px, type DM Sans 12px / `--muted`, distance right-aligned
- **Cleanliness badge on pin:** Circle 28×28px, white bg, shadow, icon 16px

### List Tab (`list_view.rs`)
- **Background:** `--bg` (#f6efe4)
- **Card:** White (`#fff`), `border-radius: 16px`, padding 14px horizontal, margin-bottom 10px, full-width minus 20px side padding
  - Avatar circle 48×48px (place-type colored), icon 22px
  - Name: DM Sans semibold 16px / `--ink`
  - Type label: DM Sans regular 12px / `--muted`
  - Distance: right-aligned, number DM Sans medium 15px / `--accent`, unit DM Sans 10px uppercase / `--muted`
  - Tag row: 3 tags side by side (cleanliness, door distance, parking)
    - Tag: `border-radius: 8px`, padding 4px 8px, icon 14px, text DM Sans medium 12px
- **Empty state:** Icon 56×56px circle (`--accent-light` bg), title DM Sans semibold 18px, subtitle DM Sans regular 14px / `--muted`, CTA button accent

### Detail View (`detail_view.rs`) — overlay sheet from map/list
- **Overlay:** Full-screen bottom sheet, `border-radius: 24px 24px 0 0`
- **Hero image area:** 180px tall, rounded top corners, gradient overlay at bottom
  - Cleanliness circle: 56×56px, white bg, shadow, icon 28px, score text DM Sans bold 18px
  - Name: Instrument Serif italic 24px / `--ink`, margin-top -30px (overlaps gradient)
  - Address: DM Sans regular 14px / `--muted`
- **Actions row:** 4 equal-width buttons, 6px gap
  - Directions: accent bg, white text, icon 20px, padding 12px, `border-radius: 14px`
  - Review / Save / More: outlined (white bg, `--border` border), icon 20px
- **Info section:** Padding 20px
  - Section title: DM Sans semibold 16px / `--ink`, margin-bottom 8px
  - Door distance: icon 18px + text DM Sans regular 14px / `--muted`
  - Hours: same style, with "Edit" link accent-colored right-aligned
- **Amenities row:** Icons in a horizontal scroll, 32×32px circles (`--accent-light` bg), icon 18px, label DM Sans medium 11px below
- **Reviews section:**
  - Avg cleanliness: large number DM Sans bold 28px / `--success`, label DM Sans regular 14px / `--muted`
  - Review cards: white bg, `border-radius: 14px`, padding 14px, margin-bottom 10px
    - Author: DM Sans semibold 14px
    - Date: DM Sans regular 12px / `--muted`
    - Stars: 5× Material Symbol star, 18px each, gold fill when lit
    - Text: DM Sans regular 14px / `--ink`, line-height 1.5
- **Write review button:** Full-width accent CTA at bottom

### Add Form (`add_form.rs`) — overlay sheet
- **Overlay:** Same as detail view
- **Title:** "Add a place" DM Sans semibold 20px
- **Subtile:** DM Sans regular 14px / `--muted`
- **Fields:** Label DM Sans medium 13px / `--muted`, input full-width white bg, border 1px `--border`, `border-radius: 12px`, padding 12px 14px, text DM Sans 15px
- **Type grid:** 2 columns, tile buttons 80×80px, `border-radius: 16px`, icon 28px, label DM Sans medium 12px below
  - Active tile: accent bg (`--accent`), white text/icon
  - Inactive tile: white bg, `--border` border
- **Amenity grid:** Same pattern as type grid but smaller tiles ~60×60px
- **Parking selector:** Horizontal pill row (3 pills)
  - Pill: `border-radius: 999px`, padding 8px 16px, icon 18px, text DM Sans medium 14px
  - Active: accent bg, white text
  - Inactive: `--border` border
- **Submit button:** Full-width accent CTA, 50px tall, `border-radius: 16px`, icon 22px + text DM Sans semibold 16px
- **Cancel link:** Centered, accent-colored, DM Sans medium 14px

### Saved Tab (`saved_view.rs`)
- Same card style as list tab (white bg, rounded, avatar circle)
- **Empty state:** Bookmark icon 56×56px, title "No saved places yet", subtitle / `--muted`
- **Unsave button:** Small outlined button on each card

### Account Tab (`account_view.rs`)
- **Background:** `--bg` (#f6efe4)
- **Header area:** Centered, padding-top 20px
  - Avatar: 72×72px circle (accent bg), initial letter DM Sans bold 28px / white
  - Name: Instrument Serif italic 24px / `--ink`
  - Username: DM Sans regular 14px / `--muted`
- **Menu items:** White cards, full-width minus padding, `border-radius: 16px`, margin-bottom 10px
  - Icon circle 40×40px (`--accent-light` bg), icon 20px
  - Label: DM Sans semibold 15px / `--ink`
  - Subtitle: DM Sans regular 13px / `--muted`
  - Chevron right: icon 20px / `--border`

### Invite Tab (`invite_view.rs`)
- Same field styles as add form
- **Invite code display:** Monospace box, `background: #f6efe4`, border 1px dashed `--border`, `border-radius: 12px`, padding 14px, text DM Sans monospace 18px / `--accent`
- **Share buttons row:** 3 icon buttons (copy/link/qr), 40×40px circles (`--accent-light` bg), icon 20px, 10px gap

### Onboarding (`onboarding.rs`)
- **Background:** Dark brown (#3a2418) — full screen overlay
- **Logo area:** Coffee cup icon 52px circle (gold bg #c9a26a), app name Instrument Serif 30px / cream, tagline DM Sans uppercase 11px / gold, letter-spacing 2.5px
- **Hero title:** Instrument Serif italic 44px / cream (#f6ecd9)
- **Subtitle:** DM Sans regular 15.5px / warm tan (#d8c3ac), line-height 1.55
- **Feature cards:** Side by side, dark brown bg (#4a3020), `border-radius: 14px`, padding 13px 14px
  - Icon 20px / gold, title DM Sans medium 12.5px / cream
- **CTA button:** Full-width, gold bg (#e7b877), dark text #3a2418, `border-radius: 16px`, padding 16px, icon 21px, text DM Sans semibold 16px
- **Fine print:** DM Sans regular 11.5px / muted tan (#a98d70)

### Register View (`register_view.rs`)
- **Background:** `--bg` (#f6efe4)
- **Title:** "Create an account" DM Sans semibold 22px, margin-bottom 6px
- **Subtitle:** DM Sans regular 14px / `--muted`, margin-bottom 6px
- **Fields:** Same as add form (label + input with lock icon prefix)
- **Submit button:** Accent CTA, same style
- **Alt auth button:** Outlined white bg, `--border` border, `border-radius: 14px`, padding 12px, icon 20px, text DM Sans semibold 15px
- **Auth note:** DM Sans regular 13px / `--muted`, centered

### Change Password View (`change_password_view.rs`)
- Same field/button styles as register view
- Back button: outlined style (alt-auth-btn)

### Admin View (`admin_view.rs`)
- Title "Admin" DM Sans semibold 22px
- Subtitle DM Sans regular 14px / `--muted`
- Export/import sections with same field/button patterns
- **Textarea:** Monospace font, 12px, min-height 140px
- **Import result:** Monospace list, 13px

### Users View (`users_view.rs`)
- Title "Users" DM Sans semibold 22px
- User rows: flex row, avatar circle 40×40px (one of 4 earth tones), initial DM Sans medium 16px / white
  - Username: DM Sans semibold 15px
  - Created date + admin badge: DM Sans regular 13px / `--muted`, admin tag DM Sans bold 11px / accent
- Border-bottom divider between rows

### Edit User View (`edit_user_view.rs`)
- Same field styles, text inputs with border
- Checkbox label: flex row, gap 8px
- **Danger zone:** Sectioned with top border, red title "Danger zone" DM Sans semibold 16px / `--danger`
- Delete button: outlined with red text and border

### Edit Place View (`edit_view.rs`) — overlay sheet
- Same overlay pattern as add form
- Type grid and amenity grid reuse same tile styles
- Requirement selector: pill row (3 options)
- **Cancel button:** Outlined, centered at bottom

### Filters Sheet (`filters_sheet.rs`)
- Scrim overlay + bottom sheet
- **Sheet handle:** 36×4px bar, `--border` color
- **Title:** "Filters" DM Sans semibold 18px
- **Reset link:** Accent text, DM Sans medium 14px (right-aligned)
- **Toggle rows:** Icon 20px + label DM Sans regular 15px / `--ink`, switch toggle right-aligned
  - Active: accent-colored switch track
  - Inactive: `--border` switch track
- **Radius slider:** Custom range input, label "Within X mi" DM Sans medium 13px / `--muted`
- **Apply button:** Full-width accent CTA

### Location Picker (`location_picker.rs`) — full-screen map overlay
- Map fills screen
- **Top bar:** Back button (icon-btn, 40×40px circle) + title "Point to the spot" DM Sans semibold 16px + subtitle DM Sans regular 13px / `--muted`
- **Bottom bar:** Coordinates display with pin icon + "Use this spot" accent CTA button

### Tab Bar (`tab_bar.rs`)
- Fixed at bottom, white bg, top shadow
- 5 tabs: Map, List, Add, Saved, Account
- Active tab: accent-colored icon + text
- Inactive: `--muted` icon + text
- **Add tab:** Special raised circle button (accent bg, white icon) centered, overlapping the bar
- Tab icons: 24px Material Symbols
- Tab labels: DM Sans regular 11px below icon
- Bar height: ~60px

### UI Helpers (`ui.rs`)
- **Badge:** Place-type colored circle, icon 18px / white
- **Stars:** 5× star icons, 20px each, gold fill (#e8a84c) when lit
- **Aspect score picker:** 5 face icons in a row, 28×28px circles
- **Clean tag:** Mop icon 14px + score DM Sans medium 12px
- **Door tag:** Walk icon 14px + distance text
- **Parking tag:** Parking icon 14px + label, color-coded by type
- **Filter chips:** Horizontal scroll row at top of map/list
  - Chip: pill shape, icon 18px, text DM Sans medium 13px, padding 7px 14px
  - Active: white bg, `--ink` text
  - Inactive: transparent, `--muted` text

---

## 3. Spacing & Sizing Summary

| Element | Size / Spacing |
|---------|---------------|
| Page horizontal padding | 20px |
| Screen title top margin | 28px (add_form), 24px (account), 20px (others) — **inconsistent** |
| Card vertical padding | 14px |
| Card horizontal padding | 14px |
| Card border-radius | 16px |
| Card margin-bottom | 10px |
| Button height (CTA) | 50-52px — **inconsistent** (add_form=52, register=50, detail actions=48) |
| Button border-radius | 14-16px — **inconsistent** |
| Input field padding | 12px 14px |
| Input border-radius | 12px |
| Input border | 1px solid `--border` |
| Field label margin-bottom | 8px (add_form), 10px (register) — **inconsistent** |
| Subtitle margin-bottom | 6-20px across pages — **highly inconsistent** |
| Section gap (between form sections) | 18-24px |
| Sheet handle height / width | 4px × 36px |
| Avatar circle (detail) | 56×56px |
| Avatar circle (list card) | 48×48px |
| Avatar circle (account) | 72×72px |
| Avatar circle (users list) | 40×40px |
| Icon size (standard) | 20px |
| Icon size (small tags) | 14-16px |
| Icon size (large/detail) | 28px |

---

## 4. Contrast Analysis (WCAG AA Requirements)

**Minimum ratios:**
- Normal text (< 18px or < 14px bold): **4.5:1**
- Large text (≥ 18px bold or ≥ 24px regular): **3:1**

### FAILURES — Below WCAG AA 4.5:1 for normal text

| Element | Foreground | Background | Ratio | Required | Status |
|---------|-----------|------------|-------|----------|--------|
| `--muted` text (#6b5a48) on `--bg` (#f6efe4) | #6b5a48 | #f6efe4 | **5.78:1** | 4.5:1 | ✅ Passes AA normal text |
| `--muted` on white cards (#fffaf3) | #6b5a48 | #fffaf3 | **6.36:1** | 4.5:1 | ✅ Passes AA normal text |
| CTA white text on accent (#934228) | #ffffff | #934228 | **5.53:1** | 4.5:1 | ✅ Passes AA normal text |
| Inactive filter chip text (#6b5a48) on `--bg` | #6b5a48 | #f6efe4 | **5.78:1** | 4.5:1 | ✅ Passes AA normal text |
| Tab bar inactive text (#6b5a48) on white | #6b5a48 | #ffffff | **6.36:1** | 4.5:1 | ✅ Passes AA normal text |
| Accent (#934228) on card-bg (#fffaf3) | #934228 | #fffaf3 | **5.33:1** | 4.5:1 | ✅ Passes AA normal text |

### PASS — Meets WCAG AA

| Element | Foreground | Background | Ratio | Notes |
|---------|-----------|------------|-------|-------|
| Primary text (#17110d) on `--bg` (#f6efe4) | #17110d | #f6efe4 | **16.38:1** | ✅ Excellent |
| Onboarding fine print (#a98d70) on dark (#3a2418) | #a98d70 | #3a2418 | **4.66:1** | ✅ Passes AA normal text |
| Onboarding subtitle (#d8c3ac) on dark (#3a2418) | #d8c3ac | #3a2418 | **8.52:1** | ✅ Excellent |
| `--danger` (#a54e2c) on `--bg` | #a54e2c | #f6efe4 | **4.93:1** | ✅ Passes AA normal text |
| Stars gold (#e8a84c) on white | #e8a84c | #fffaf3 | **3.12:1** | ⚠️ Passes large-text (3:1), fails normal — stars are decorative, OK |

### The Core Problem

The `--muted` color (`#6b5a48`) achieves **5.78:1** against cream and **6.36:1** against white — comfortably passing WCAG AA normal-text requirement (4.5:1) across all secondary text uses.

The accent color (`#934228`) reaches **5.33:1** on card backgrounds and **5.53:1** for white text on accent — both pass AA.

---

## 5. Consistency Issues Found

### A. Spacing Inconsistencies
- Screen title top margin varies: 20px (map sheet), 24px (account), 28px (add_form, detail), no explicit on some pages
- Subtitle `margin-bottom` varies wildly: 6px (register), 10px (detail hero), 14px (users), 18px (admin, invite)
- Field label `margin-bottom`: 8px vs 10px between add_form and register

### B. Button Size Inconsistencies
- CTA button height: 48px (detail actions), 50px (register/change-password), 52px (add_form)
- Border-radius on buttons: 14px vs 16px
- Some pages use `submit-btn` class, others inline styles for alt-auth buttons

### C. Avatar Size Inconsistencies
- No unified avatar system — sizes range from 40px to 72px depending on context
- Should have a scale: sm (32), md (40), lg (48/56), xl (72)

### D. Missing / Ad-hoc Styles
- Several components use inline `style="..."` for margins/padding instead of CSS classes
  - `add_form.rs`: inline `margin-bottom: 18px`, `padding-top: 18px`
  - `account_view.rs`: inline `margin-bottom: 20px`, `gap: 14px`
  - `register_view.rs`: inline `margin-bottom: 6px`
  - `admin_view.rs`: inline `margin-bottom: 18px`, `margin-top: 18px`
  - `edit_user_view.rs`: inline `margin-bottom: 14px`, `padding-top: 18px`
- This makes it hard to adjust spacing globally

### E. Font Size Inconsistencies
- Sheet titles: 18px (map sheet, filters) vs 20px (add form) vs 22px (screen-title class)
- Card names: 16px consistently ✅
- Friend name in account: 18px vs user row username: 15px

### F. Icon Size Inconsistencies
- Standard icon is 20px but appears at 14px, 16px, 18px, 22px, 24px, 26px, 28px across components
- No unified icon size scale in CSS

---

## 6. Recommended Fixes (Priority Order)

### P0 — Contrast (WCAG AA Compliance)

1. ~~Darken `--muted`~~ — already done (`#6b5a48`, ratio 5.78:1 on cream) ✅
2. ~~Darken accent `--accent`~~ — already done (`#934228`, ratio 5.33:1 on card-bg) ✅
   
3. Onboarding fine print (`#a98d70` on `#3a2418`) already passes at **4.66:1** ✅ — no change needed

### P1 — Spacing System (standardize the ad-hoc inline margins)

3. Add CSS spacing classes and replace inline `style="margin-bottom: X"` throughout:
   - `.screen-padding` → 20px horizontal
   - `.section-gap` → 24px vertical gap between sections  
   - `.field-gap` → 16px vertical gap between form fields
   - `.card-gap` → 10px vertical gap between cards
   - Title margin scale: use `.screen-title` with consistent 24-28px top margin

### P2 — Button Normalization

4. Standardize CTA (`submit-btn`) height to **50px**, `border-radius: 16px`, padding 0
5. Standardize outlined button (`alt-auth-btn`) height to **46px**, `border-radius: 14px`
6. Replace inline style margins on buttons with class-based spacing

### P3 — Typography & Icon Scale

7. Sheet titles → unify to **20px** DM Sans semibold (currently 18-22px across pages)
8. Create icon size classes: `.mi-sm` (16px), `.mi-md` (20px), `.mi-lg` (28px)
9. Avatar sizes: define `.avatar-sm` (32px), `.avatar-md` (40px), `.avatar-lg` (56px), `.avatar-xl` (72px)

### P4 — Replace Inline Styles with Classes

10. Convert all inline `style="margin-*"` and `style="padding-*"` in component HTML to CSS classes
11. This enables global theme adjustments from one place and eliminates the spacing inconsistencies documented in Section 5A
