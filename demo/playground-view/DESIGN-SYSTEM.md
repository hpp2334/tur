# tur Playground — Design System

**Status**: Normative · **Scope**: `@tur-ng/playground-view` (the playground shell — sidebar, editor, viewer, error chrome) · **Theme**: Light

This document is the single source of truth for visual decisions in the tur playground. Every color, size, and view shape in `src/index.ts` and `src/compile.ts` must trace back to a token defined here. See [`STYLE-GUIDE.md`](./STYLE-GUIDE.md) for how to apply these tokens.

---

## 1. Foundations

### 1.1 Color — primitive palette

Primitives are organized by family with a numeric step. **Never use primitives directly in views** — go through semantic tokens (§2). Primitives exist so the semantic layer can be rethemed without touching view code.

#### Neutrals — `tur.ink.*`

Warm-tinted light grays. Slight blue-green bias (not pure gray) to feel "engineered" rather than corporate.

| Token | Hex | Contrast on `ink.50` | Role |
|---|---|---|---|
| `ink.50`  | `#fbfcfd` | — | App background (lightest) |
| `ink.100` | `#f4f6f9` | — | Panel background |
| `ink.150` | `#eceff4` | — | Elevated surface / header |
| `ink.200` | `#e1e5ec` | — | Hover surface |
| `ink.300` | `#d4d9e0` | — | Selected surface (muted), subtle border |
| `ink.400` | `#b8c0cc` | 1.9:1 | Strong border, placeholder text |
| `ink.500` | `#8a94a3` | 3.2:1 | Tertiary text, code comments (AA large/UI only) |
| `ink.600` | `#5e6878` | 5.7:1 | Secondary text ✓ AA |
| `ink.700` | `#3a4250` | 9.6:1 | Body text ✓ AAA |
| `ink.800` | `#1f2530` | 15.9:1 | Primary text ✓ AAA |
| `ink.900` | `#0a0e14` | 19.8:1 | Highest contrast; text on accent fills |

#### Brand — `tur.teal.*`

Saturated cyan-leaning teal. The signature. Seven steps because light theme needs both bright fills (with dark text) and dark text-safe variants (for links, focus indicators on white).

| Token | Hex | Use |
|---|---|---|
| `teal.200` | `#7df5d0` | Soft selection background, decorative tint |
| `teal.300` | `#00e8b8` | Hover fill on bright surfaces, decorative glow |
| `teal.400` | `#00c69a` | **Primary fill** — use with `ink.900` text (8.5:1) |
| `teal.500` | `#00a886` | Solid brand color, cursor, default accent fill |
| `teal.600` | `#008a6e` | Border on accent surfaces, focus ring outer |
| `teal.700` | `#006e58` | Pressed dark, secondary accent text |
| `teal.800` | `#005440` | Accent-as-text on light (9.3:1) — links, focus text |

**Critical pattern**: primary buttons use **`ink.900` text on `teal.400` fill** (8.5:1 contrast). Do **not** put white text on bright teal — white on `teal.400` is only 2.4:1 and fails AA.

#### Warm complement — `tur.coral.*`

Used sparingly for emphasis, onboarding highlights, secondary CTAs. **Not** for errors.

| Token | Hex | Use |
|---|---|---|
| `coral.300` | `#ffb3a3` | Soft highlight tint |
| `coral.400` | `#ff8a72` | Solid fill (with `ink.900` text) |
| `coral.500` | `#e85d44` | Text-on-light (3.6:1 AA-large; use `coral.700` for body) |
| `coral.700` | `#b03a1f` | Body-length coral text (5.4:1 ✓ AA) |

#### Semantic states — `tur.status.*`

Verified AA on `ink.50` for both text (4.5:1+) and large UI (3:1+) use.

| Token | Hex | Text contrast on `ink.50` | Use |
|---|---|---|---|
| `status.success` | `#00a06b` | 4.5:1 ✓ | Ready state, confirmations |
| `status.warning` | `#c47700` | 4.7:1 ✓ | Warnings, stale data |
| `status.error`   | `#d63a2f` | 5.1:1 ✓ | Errors, destructive actions |
| `status.info`    | `#006e58` | 6.8:1 ✓ | Info notices (reuses `teal.700`) |

For status **fills** (badges, dots) use the same hex with `ink.900` text — all four pass AA at 3:1+ for large text.

#### Code surface — `tur.code.*`

Replaces the current One Dark palette in `compile.ts`. Tuned for the light theme: every token passes AA on `code.bg` for body-length reading.

| Token | Hex | Token type |
|---|---|---|
| `code.bg` | `#fbfcfd` | Editor background (same as `ink.50`) |
| `code.fg` | `#1f2530` | Default text (`ink.800`) |
| `code.keyword` | `#006e58` | Keywords (`teal.700`) |
| `code.string` | `#3f7d3f` | Strings |
| `code.number` | `#b35900` | Numeric literals |
| `code.comment` | `#8a94a3` | Comments (`ink.500`, italic) |
| `code.operator` | `#5e6878` | Operators and punctuation (`ink.600`) |
| `code.literal` | `#92400e` | `true` / `false` / `null` |

### 1.2 Typography

**Font families**

| Token | Family | Stack fallback | Use |
|---|---|---|---|
| `font.ui` | Inter | `-apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif` | All chrome, labels, body |
| `font.mono` | JetBrains Mono | `"SF Mono", Menlo, Consolas, "Roboto Mono", monospace` | Editor, code, keyboard hints |

> Engine note: if the runtime cannot load bundled fonts, system fallbacks apply automatically. Do not introduce a third family.

**Type scale**

| Token | Size / Line / Weight | Use |
|---|---|---|
| `type.caption` | 10 / 1.4 / 500 | Keyboard hints (`⌘S`), timestamp meta |
| `type.micro`   | 11 / 1.45 / 500 | Badges, status labels, secondary meta |
| `type.body`    | 13 / 1.55 / 400 | UI body, nav items, list rows |
| `type.title`   | 14 / 1.4 / 600  | Panel headers, section titles |
| `type.display` | 18 / 1.2 / 700  | Top-level titles (rare — app title only) |
| `type.code`    | 13 / 1.6 / 400 mono | Editor and inline code |

**Rules**
- Never use a size outside this scale.
- Never use a weight other than 400, 500, 600, 700.
- Italic is reserved for code comments only.
- UI text is sentence case (`Ready`, not `READY` or `ready`).

### 1.3 Spacing

Base unit: **4px**. All paddings, margins, gaps, and offsets use this scale.

| Token | px | Common use |
|---|---|---|
| `space.0`  | 0  | Flush |
| `space.xs` | 4  | Tight icon-to-label, dense list padding |
| `space.sm` | 8  | Default inner padding for chips, list rows |
| `space.md` | 12 | Panel inner padding, header vertical |
| `space.lg` | 16 | Default content padding |
| `space.xl` | 24 | Section spacing, sidebar header padding |
| `space.2xl`| 32 | Major region separation |
| `space.3xl`| 48 | Hero / empty-state padding |

### 1.4 Radii

| Token | px | Use |
|---|---|---|
| `radius.0`    | 0   | Full-bleed panels, app shell |
| `radius.sm`   | 4   | Badges, small chips |
| `radius.md`   | 6   | Buttons, inputs |
| `radius.lg`   | 8   | Cards, modals, overlays |
| `radius.full` | 999 | Status dots, pills |

### 1.5 Elevation

Light theme shadows are subtle — most separation is done with **background-color shifts**, not shadow.

| Token | Value | Use |
|---|---|---|
| `elev.0` | none | Flat panel against app bg — separate by `bg.panel` ≠ `bg.app` |
| `elev.1` | `0 0 0 1px <border.subtle>` | Subtle outline (default for elevated panels) |
| `elev.2` | `0 1px 3px rgba(15,20,26,0.08), 0 4px 12px rgba(15,20,26,0.06)` | Floating cards, dropdowns |
| `elev.3` | `0 8px 24px rgba(15,20,26,0.12)` | Modals, command palette |

**Focus ring** (always drawn outside the element): `0 0 0 2px <bg.app>, 0 0 0 4px <teal.600>`. The inner ring prevents the outer from bleeding into the element's own pixels.

### 1.6 Motion

| Token | Duration / Easing | Use |
|---|---|---|
| `motion.fast` | 120ms `ease-out` | Hover/active color changes, selection highlight |
| `motion.med`  | 200ms `ease-out` | Panel transitions, expand/collapse |
| `motion.slow` | 320ms `ease-in-out` | Modal/overlay entrance, large layout shifts |

**Rules**
- No motion over 400ms.
- No linear easing for UI transitions — always `ease-out` (entrances) or `ease-in-out` (state changes).
- Respect `prefers-reduced-motion`: collapse all `motion.*` to 0ms. (Engine support TBD — flag for phase 5.)

### 1.7 Density

- **Compact** is the default (this is a developer tool, not a marketing site).
- Minimum click target: **28px height**. The current sidebar item (`padding: 8` + `fontSize: 12`) ≈ 28px — keep.
- Panel headers: 36px tall (`padding: 8` vertical + `type.title` 14px line-height ≈ 20px → 36px).
- Editor/viewer gutters: `space.md` (12px) on all sides.

---

## 2. Semantic token table

**This is the layer views use.** Each entry maps to one or more primitives.

### Background

| Token | Maps to | Use |
|---|---|---|
| `bg.app`          | `ink.50`  | Root background behind all panels |
| `bg.panel`        | `ink.100` | Sidebar, editor, viewer base |
| `bg.elevated`     | `ink.150` | Panel headers, sticky bars |
| `bg.hover`        | `ink.200` | Hover state for rows, items, buttons (ghost) |
| `bg.selected`     | `teal.200` | Selected nav item, current case highlight |
| `bg.header`       | `ink.150` | Panel header strip (alias of `bg.elevated`) |
| `bg.code`         | `ink.50`  | Editor surface (slightly lighter than panel for focus) |
| `bg.viewer`       | `ink.50`  | Viewer canvas background (alias of `bg.code`) |
| `bg.danger`       | `#fff0ee` | Error banner background |
| `bg.button.primary`   | `teal.400` | Primary button (with `text.onAccent`) |
| `bg.button.secondary` | `ink.200`  | Secondary button (with `text.primary`) |
| `bg.button.ghost`     | transparent | Ghost button (with `text.secondary`) |

### Text

| Token | Maps to | Use |
|---|---|---|
| `text.primary`    | `ink.800` | Headlines, primary labels |
| `text.body`       | `ink.700` | Body copy, list items |
| `text.secondary`  | `ink.600` | Section labels, meta |
| `text.tertiary`   | `ink.500` | Placeholder, disabled hints |
| `text.disabled`   | `ink.400` @ 60% alpha | Disabled controls |
| `text.onAccent`   | `ink.900` | Text on `teal.400`/`teal.500` fills |
| `text.onDanger`   | `#8a1a14` | Text on `bg.danger` |
| `text.placeholder`| `ink.400` | Input placeholder |
| `text.code`       | `code.fg` | Editor and inline code |
| `text.link`       | `teal.800` | Hyperlinks (in MD-rendered text) |

### Border

| Token | Maps to | Use |
|---|---|---|
| `border.subtle` | `ink.300` | Default 1px separators |
| `border.strong` | `ink.400` | Emphasized separators (panel edges) |
| `border.focus`  | `teal.600` | Focus ring outer (see §1.5) |

### Accent

| Token | Maps to | Use |
|---|---|---|
| `accent.primary` | `teal.400` (fills) / `teal.800` (text) | Default accent — pick by context |
| `accent.solid`   | `teal.500` | Non-interactive brand fill (brand mark, selection cursor) |
| `accent.cursor`  | `teal.500` | Caret in editor |
| `accent.complement` | `coral.400` (fills) / `coral.700` (text) | Secondary accent — use sparingly |

### Status

| Token | Maps to | Use |
|---|---|---|
| `status.success` | `#00a06b` | Ready badge, success dot |
| `status.warning` | `#c47700` | Stale data, caution |
| `status.error`   | `#d63a2f` | Error badge, error dot, destructive CTA text |
| `status.info`    | `#006e58` | Info notices |

---

## 3. Layout

### 3.1 Shell — toolbar + three-pane body + status bar

```
┌─────────────────────────────────────────────────────────────────┐
│  Toolbar · ~40px                                                │
│  [tur] playground   ▸ counter    [▶ Run] [↺ Reset] [auto ◯] [ Split | Edit | View ]
├──────────┬──────────────────────────┬───────────────────────────┤
│          │                          │                           │
│ Sidebar  │      Editor              │      Viewer               │
│ 200px    │      Expanded (flex N)   │      Expanded (flex M)    │
│ (cases)  │                          │      (render OR error)    │
│          │                          │                           │
├──────────┴──────────────────────────┴───────────────────────────┤
│  Status bar · ~24px                                             │
│  ● ready   ◉ edited   ⇡ compiled 2s ago   ⌘S to run     tur v0.1│
└─────────────────────────────────────────────────────────────────┘
```

- **Toolbar**: consolidates brand, current case name, and primary actions (Run, Reset, Auto-run toggle, Layout mode). Always visible.
- **Sidebar**: fixed `200px` width, scrollable list of cases. Hover and selected states per §5.
- **Editor & Viewer**: each `Expanded({ flex })`. Flex ratio is bound to `layoutMode$` — `split` 1:1, `editor` 2:1, `viewer` 1:2.
- **Status bar**: single source of truth for runtime state. Always visible.
- **Minimum viewport**: none — the playground is responsive. At ≥720px CSS width it uses the desktop 3-pane layout; below 720px it collapses to a single full-width pane switched by a bottom tab bar (Cases / Edit / View). The breakpoint is driven by the engine-owned `viewportSize$` atom (`isMobile$ = derive(() => get(viewportSize$).width < 720)`), which the engine syncs each frame from the canvas resize.
- **Gutters**: none between panes — separation is by background-color shift (`bg.app` → `bg.panel` → `bg.code`/`bg.viewer` → `bg.elevated` for chrome).

### 3.2 Layout modes

Driven by `layoutMode$: Source<"split" | "editor" | "viewer">`. The EditorAndViewer container reads the source via `derive()` and applies the matching flex values to its two `Expanded` children — no `Switch` rebuild needed, so editor cursor and viewer state survive mode changes.

| Mode | Editor flex | Viewer flex | Use |
|---|---|---|---|
| `split` (default) | 1 | 1 | General editing + preview |
| `editor` | 2 | 1 | Writing code, peeking at result |
| `viewer` | 1 | 2 | Inspecting render, minor edits |

### 3.3 Density zones

| Zone | Padding | Examples |
|---|---|---|
| Tight | `space.xs` (4) | Status bar contents, segmented-control buttons |
| Default | `space.sm` (8) | Sidebar items, badges, toolbar buttons |
| Comfortable | `space.md` (12) | Editor body, viewer body, toolbar outer |
| Spacious | `space.xl` (24) | Error panel centering |

---

## 4. View catalog

Each view below must be extracted into `src/views/` during phase 3 of the roadmap (§9). Specs are normative — any deviation needs design review.

### 4.1 `Shell`

The root layout. Holds Sidebar + Editor + Viewer in a `Row`.

| Prop | Type | Default | Notes |
|---|---|---|---|
| `sidebar` | `Element` | required | Typically `<NavList>` |
| `editor` | `Element` | required | Typically `<EditorSurface>` |
| `viewer` | `Element` | required | Typically `<ViewerSurface>` |

**Background**: `bg.app`. **Padding**: none. **Children layout**: `Row`, sidebar fixed `220px`, editor and viewer each `Expanded`.

```tsx
Container({
    color: tokens.bg.app,
    children: [Row({
        children: [
            sidebar,
            Expanded({ child: editor }),
            Expanded({ child: viewer }),
        ],
    })],
})
```

### 4.2 `Panel`

Reusable header + body container.

| Prop | Type | Default | Notes |
|---|---|---|---|
| `title` | `Val<string>` | — | Left-aligned in header |
| `actions` | `Element[]` | `[]` | Right-aligned row |
| `body` | `Element` | required | Fills remaining space |
| `headerless` | `boolean` | `false` | Hide header strip entirely |

**Colors**: header = `bg.header`, body = `bg.panel`. **Padding**: header `space.sm` horizontal × `space.sm` vertical; body `space.md`. **Border**: 1px `border.subtle` between header and body (only when header visible).

```tsx
Container({
    color: tokens.bg.panel,
    children: [Column({
        crossAlignment: CrossAxisAlignment.Stretch,
        children: [
            Container({
                padding: 8, color: tokens.bg.header,
                children: [Row({
                    mainAlignment: MainAxisAlignment.SpaceBetween,
                    children: [
                        Text({ text: title, fontSize: 14, fontWeight: 600,
                               color: tokens.text.primary }),
                        Row({ children: actions }),
                    ],
                })],
            }),
            Expanded({ child: Container({ padding: 12, children: [body] }) }),
        ],
    })],
})
```

### 4.3 `NavItem`

A single selectable item in the sidebar.

| Prop | Type | Default | Notes |
|---|---|---|---|
| `label` | `Val<string>` | required | Case name |
| `selected$` | `Readable<boolean>` | — | Reactive selected state |
| `onClick` | `Mutation` | — | Selection handler |
| `icon` | `Element` | — | Optional leading icon (16×16) |

**States** (see §5 for full matrix):

| State | Background | Text color |
|---|---|---|
| default | `bg.panel` | `text.body` |
| hover | `bg.hover` | `text.primary` |
| selected | `bg.selected` (`teal.200`) | `text.primary` |
| selected+hover | `teal.300` | `text.primary` |
| disabled | `bg.panel` | `text.disabled` |

**Padding**: `space.sm` (8). **Radius**: 0 (full-bleed in sidebar). **Click target**: min 28px height — already satisfied at `padding: 8` + `type.body`.

```tsx
PointerInteract({
    onClick,
    onPointerEnter: hover$on,
    onPointerExit: hover$off,
    child: Container({
        padding: 8,
        color: derive(() => get(selected$)
            ? (get(hover$) ? tokens.teal[300] : tokens.teal[200])
            : (get(hover$) ? tokens.bg.hover : tokens.bg.panel)),
        children: [Text({
            text: label, fontSize: 13,
            color: derive(() => get(selected$)
                ? tokens.text.primary
                : tokens.text.body),
        })],
    }),
})
```

### 4.4 `NavList`

Vertical list of `NavItem`s inside a `ScrollView`. Used by the sidebar body.

| Prop | Type | Default | Notes |
|---|---|---|---|
| `items` | `{ label, key }[]` | required | Drives an `Each` |
| `selectedKey$` | `Readable<string>` | — | Currently selected key |
| `onSelect` | `Mutation<[string]>` | — | Receives selected key |
| `header` | `Element` | — | Optional leading element (app title) |

### 4.5 `Button`

Three variants, same shape.

| Prop | Type | Default | Notes |
|---|---|---|---|
| `variant` | `"primary" \| "secondary" \| "ghost"` | `"secondary"` | See state table |
| `label` | `Val<string>` | required | Always sentence case |
| `icon` | `Element` | — | Optional leading icon |
| `onClick` | `Mutation` | — | |
| `disabled` | `Val<boolean>` | `false` | |

**Shape**: `radius.md`, `padding: space.xs horizontal × space.sm vertical`, `type.body`.

| Variant | default bg | default text | hover bg | pressed bg |
|---|---|---|---|---|
| primary | `bg.button.primary` (`teal.400`) | `text.onAccent` (`ink.900`) | `teal.300` | `teal.500` |
| secondary | `bg.button.secondary` (`ink.200`) | `text.primary` | `ink.300` | `ink.400` |
| ghost | transparent | `text.secondary` | `bg.hover` | `ink.300` |

```tsx
PointerInteract({
    onClick,
    onPointerEnter: hover$on,
    onPointerExit: hover$off,
    child: Container({
        padding: [4, 8], borderRadius: 6,
        color: derive(() => buttonColorFor(variant, hover$, pressed$)),
        children: [Row({ children: [
            ...(icon ? [icon] : []),
            Text({ text: label, fontSize: 13, fontWeight: 500,
                   color: derive(() => buttonTextColorFor(variant)) }),
        ]})],
    }),
})
```

### 4.6 `StatusBadge`

A status dot + label, used in panel headers.

| Prop | Type | Default | Notes |
|---|---|---|---|
| `status` | `Val<"ready" \| "error" \| "warning" \| "info">` | required | |
| `label$` | `Val<string>` | derived from `status` | Override for custom label |

**Shape**: dot `radius.full` 6×6, gap `space.xs`, label `type.micro`.

| Status | Dot color | Label color |
|---|---|---|
| ready | `status.success` | `status.success` |
| error | `status.error` | `status.error` |
| warning | `status.warning` | `status.warning` |
| info | `status.info` | `status.info` |

### 4.7 `EditorSurface`

Code editor pane: header (case name + Run button) + scrollable editor body.

Wraps `Input` with the canonical editor config.

| Prop | Type | Default | Notes |
|---|---|---|---|
| `controller` | `TextEditingController` | required | |
| `caseName$` | `Readable<string>` | — | Shown in header |
| `onRun` | `Mutation` | — | Cmd-S or Run button |

**Editor field config** (normative):

```tsx
Input({
    controller,
    multiline: true,
    fontFamily: "mono",      // engine resolves to JetBrains Mono fallback
    fontSize: 13,            // type.code
    color: tokens.text.code,           // code.fg
    cursorColor: tokens.accent.cursor, // teal.500
    placeholderColor: tokens.text.placeholder,
})
```

**Background**: `bg.code`. **Padding**: `space.md`. **Header**: `<Panel>` header strip with `actions: [<Button variant="primary" label="Run" icon={PlayIcon} onClick={onRun} />]`.

### 4.8 `ViewerSurface`

Rendered-case pane: header (status badge) + body (case content) + optional error banner.

| Prop | Type | Default | Notes |
|---|---|---|---|
| `status$` | `Readable<"ready" \| "error">` | — | Drives `<StatusBadge>` |
| `child$` | `Readable<Element>` | — | The case view to render |
| `error$` | `Readable<string>` | — | When non-empty, shows `<ErrorBanner>` |

**Background**: `bg.viewer`. **Body**: case view inside `Expanded`. **Error overlay**: pinned to bottom of body, `Condition` on `error$`.

### 4.9 `ErrorBanner`

Inline error display at the bottom of the viewer.

| Prop | Type | Default | Notes |
|---|---|---|---|
| `message$` | `Readable<string>` | required | |

**Background**: `bg.danger` (`#fff0ee`). **Text**: `text.onDanger` (`#8a1a14`), `type.micro`. **Padding**: `space.sm`. **Border-top**: 1px `status.error` @ 30% alpha. **Icon**: leading `!` in a 16×16 `radius.full` `status.error` circle (white `!`).

### 4.10 `Placeholder`

Empty-state shown when no case is selected or a case fails to compile without a runtime error.

| Prop | Type | Default | Notes |
|---|---|---|---|
| `message` | `Val<string>` | `"(no case)"` | |

**Background**: `bg.viewer`. **Text**: `text.tertiary`, `type.body`, centered (`Alignment.Center`). **Padding**: `space.3xl`.

### 4.11 `Toolbar` (implemented)

Top chrome strip — brand, current case name, primary actions. Single horizontal `Row` with three regions; outer Container provides background and bottom border.

| Region | Content | Width |
|---|---|---|
| Left | Brand: `tur` (teal.500, 14px) + `playground` (text.secondary, 11px) | content (`mainAxisSize: Min`) |
| Center | Current case name (text.body, 13px) | `Expanded` (fills remaining) |
| Right | RunButton + ResetButton + AutoRunToggle + LayoutControl | content (`mainAxisSize: Min`) |

**Background**: `bg.elevated`. **Border**: 1px `border.subtle` (visible only on bottom edge — top/left/right sit at canvas bounds). **Padding**: `space.sm` vertical × `space.md` horizontal. **Approximate height**: 40px.

> **Engine note**: every inner `Row` in the toolbar MUST set `mainAxisSize: MainAxisSize.Min`. Default `Max` causes each region to consume the parent's full width, pushing other regions off-screen.

### 4.12 `RunButton` (implemented)

Primary CTA. Triggers `recompile()`. Always visible.

| State | Background | Text |
|---|---|---|
| default | `bg.button.primary` (teal.400) | `text.onAccent` (ink.900) |
| hover | `bg.button.primaryHover` (teal.300) | `text.onAccent` |
| pressed | `bg.button.primaryPressed` (teal.500) | `text.onAccent` |

**Shape**: `radius.md` (6), `padding: 6` all sides. **Label**: `▶` glyph (9px) + `Run` (12px) in a `mainAxisSize: Min` Row. **Hover state** via shared `runHovered$` source.

### 4.13 `ResetButton` (implemented)

Secondary action. Reverts editor to selected case's original source and recompiles. No confirmation.

| State | Background | Text |
|---|---|---|
| default | `bg.button.ghost` (ink.50) | `text.secondary` |
| hover | `bg.hover` (ink.200) | `text.secondary` |

**Shape**: same as RunButton. **Label**: `↺` glyph (12px) + `Reset` (12px).

### 4.14 `Toggle` (implemented — currently inlined as `AutoRunToggle`)

Pill-shaped boolean switch. Built from `PointerInteract` + `Container` + `Stack` + `Positioned` — the engine has no built-in toggle view.

| Prop | Type | Notes |
|---|---|---|
| `value$` | `Readable<boolean>` | Reactive state |
| `onChange` | `Mutation` | Receives no args; flip the source inside |

**Shape**: 28×16 outer Container, `radius.full`. Knob: 12×12 `Container`, `radius.full`, `text.inverse` (ink.50). Knob position: `Positioned.left = value ? 14 : 2`, `top: 2`. **Background**: `bg.button.primary` (teal.400) when on, `bg.hover` (ink.200) when off. **Animation**: none in v1 — knob snaps. Future: 120ms ease-out via `createAnimationController`.

### 4.15 `SegmentedControl` (implemented — currently inlined as `LayoutControl`)

A row of mutually-exclusive option buttons. Selected option uses inverted background ("pressed-in" look).

| Prop | Type | Notes |
|---|---|---|
| `value$` | `Readable<string>` | Currently-selected key |
| `options` | `{ key, label }[]` | Typically 2-4 options |
| `onChange` | `Mutation<[string]>` | Receives new key |

**Tray background**: `bg.controlTray` (ink.200). **Option padding**: `space.xs` (6). **Option states**:

| State | Background | Text |
|---|---|---|
| default | `bg.controlTray` (matches tray — invisible) | `text.secondary` |
| hover (not selected) | `bg.controlTrayHover` (ink.300) | `text.secondary` |
| selected | `bg.controlSelected` (ink.50) | `text.primary` |

**Hover tracking**: single `hoveredOption$` source keyed by option key. **No radii** — children sit flush in the tray.

### 4.16 `StatusBadge` (implemented — inlined as `StatusDot` + label)

Status dot + label pair. Used in the status bar.

| Status | Dot color | Label color | Label text |
|---|---|---|---|
| ready | `status.success` | `status.success` | `ready` |
| error | `status.error` | `status.error` | `error` |
| warning | `status.warning` | `status.warning` | `warning` |
| info | `status.info` | `status.info` | `info` |

**Dot**: 6×6 `Container`, `radius.full`. **Label**: `type.micro` (11px). **Gap**: `space.xs` (6).

### 4.17 `StatusBar` (implemented)

Bottom chrome strip. Always-visible runtime state. Outer Container with 1px top border.

| Cluster | Contents |
|---|---|
| Left | StatusDot + label, edited pill (Condition on `edited$`), "compiled {relative}" timestamp |
| Right | Keyboard hint ("⌘S to run" when auto-run off, "auto-run on" when on), version label |

**Background**: `bg.elevated`. **Border**: 1px `border.subtle` (visible only on top edge). **Padding**: `space.xs` (4) vertical × `space.sm` (8) horizontal. **Approximate height**: 24px.

> **Reactive pattern**: the relative timestamp uses two sources — `lastCompiledAtMs$` (set on every successful recompile) and `now$` (ticks every 5s via a `launch` loop driving `yield sleep(5000)`). The displayed text is `derive`d from both. This avoids needing to manually invalidate the timestamp.

### 4.18 `ErrorPanel` (implemented)

Full-panel error state. Replaces the Viewer body when `status$ === "error"` (not an overlay — the rendered content is disposed).

| Prop | Type | Notes |
|---|---|---|
| `message$` | `Readable<string>` | Compiler error message |

**Background**: `bg.danger`. **Alignment**: center. **Contents** (vertical Column):
- 32×32 `radius.full` `status.error` circle with white `!` (18px), centered
- `space.xl` (16) gap
- "Compile error" title — `type.title` (14px), `text.onDanger`
- `space.sm` (8) gap
- Error message — `type.body` (12px), `text.onDanger`, monospace preferred

**Padding**: `space.xl` (24). **No dismiss button** — fixing the code and recompiling clears the state automatically.

---

## 5. State matrix

Every interactive view must define all five states. Hover/active require `PointerInteract` with `onPointerEnter`/`onPointerExit` (already in the API — see `PointerInteractProps` in `js/packages/tur-std/src/index.d.ts`). Focus requires future keyboard support; spec it now so it can drop in.

| View | default | hover | active/pressed | focused | disabled |
|---|---|---|---|---|---|
| NavItem | panel/body | hover/primary | teal.300/primary | focus ring | panel/disabled |
| Button primary | teal.400/onAccent | teal.300/onAccent | teal.500/onAccent | focus ring | ink.200/disabled |
| Button secondary | ink.200/primary | ink.300/primary | ink.400/primary | focus ring | ink.100/disabled |
| Button ghost | transparent/secondary | hover/primary | ink.300/primary | focus ring | transparent/disabled |
| RunButton | teal.400/onAccent | teal.300/onAccent | teal.500/onAccent | focus ring | — |
| ResetButton | ink.50/secondary | ink.200/secondary | ink.300/secondary | focus ring | — |
| Toggle (on) | teal.400 | — | — | focus ring | — |
| Toggle (off) | ink.200 | — | — | focus ring | — |
| SegmentedControl option (selected) | ink.50/primary | — | — | focus ring | — |
| SegmentedControl option (not selected) | transparent/secondary | ink.300/secondary | — | focus ring | — |
| StatusBadge | per status | — | — | — | — |
| Editor field | per tokens | — | — | accent.cursor + 1px border.focus | — |

**Focus ring** is always the double-ring from §1.5. It must be visible against any background — that's why the inner ring uses `bg.app`.

---

## 6. Iconography

**System**: line icons in the [Lucide](https://lucide.dev) style (24×24 source grid, 1.5px stroke at 24px, rounded joins). Rendered via the `Svg` element with preloaded resource IDs.

**Sizes**:
- `icon.sm` 12×12 — inline with `type.caption`
- `icon.md` 16×16 — default, pairs with `type.body`/`type.micro`
- `icon.lg` 20×20 — prominent CTAs, empty states

**Color**: inherits text color by default; uses `currentColor` equivalent (set stroke at the SVG level). For status icons, use the corresponding `status.*` token.

**Required icon set** (phase 5):
| Icon | Use |
|---|---|
| `play` | Run button |
| `rotate-ccw` | Reset |
| `alert-triangle` | Error banner |
| `check` | Ready badge (optional) |
| `chevron-right` | Selected nav item (optional indicator) |

**Engine note**: SVG requires `createImageResource(bytes)` → pass the returned id to `Svg({ resourceId, width, height })`. Bundle as UTF-8 strings; decode at startup.

---

## 7. Accessibility

### 7.1 Contrast

All token-to-token pairs in §2 meet WCAG AA (4.5:1 for body text, 3:1 for UI/large). Documented exceptions:
- `text.tertiary` (`ink.500`): 3.2:1 — use **only** for ≥14px text or non-text UI (placeholder, captions under 11px are prohibited).
- `coral.500` as text: 3.6:1 — large text only. For body-length coral text use `coral.700`.

### 7.2 Focus

Every interactive element shows the §1.5 focus ring when focused. No `outline: none` equivalents — focus is part of the design, not an exception.

### 7.3 Keyboard

The playground must be fully navigable by keyboard:
- `Tab` / `Shift+Tab`: cycle through Sidebar → Editor → Run button → Viewer.
- `Enter` / `Space`: activate focused item.
- `↑` / `↓`: move within sidebar without losing focus.
- `Cmd+S`: recompile (already implemented).
- `Cmd+/`: comment toggle in editor (future).

### 7.4 Motion

Honor `prefers-reduced-motion`. All `motion.*` durations collapse to 0ms. (Requires engine media-query support — phase 5.)

### 7.5 Color-only signaling

Never signal state with color alone. `<StatusBadge>` always pairs its colored dot with a text label. Error banners always include a `!` glyph, not just red.

---

## 8. Token implementation

Tokens live in **`src/tokens.ts`**. The file is the only place `Color.hex(...)` is allowed outside of `compile.ts`.

```ts
// src/tokens.ts
import { Color } from "tur:std";

// Primitive palette — do not import these from views.
export const ink = {
    50: Color.hex("#fbfcfd"), 100: Color.hex("#f4f6f9"), 150: Color.hex("#eceff4"),
    200: Color.hex("#e1e5ec"), 300: Color.hex("#d4d9e0"), 400: Color.hex("#b8c0cc"),
    500: Color.hex("#8a94a3"), 600: Color.hex("#5e6878"), 700: Color.hex("#3a4250"),
    800: Color.hex("#1f2530"), 900: Color.hex("#0a0e14"),
} as const;

export const teal = {
    200: Color.hex("#7df5d0"), 300: Color.hex("#00e8b8"), 400: Color.hex("#00c69a"),
    500: Color.hex("#00a886"), 600: Color.hex("#008a6e"), 700: Color.hex("#006e58"),
    800: Color.hex("#005440"),
} as const;

// ... coral, status, code similarly

// Semantic layer — views import from here.
export const tokens = {
    bg: {
        app: ink[50], panel: ink[100], elevated: ink[150], hover: ink[200],
        selected: teal[200], header: ink[150], code: ink[50], viewer: ink[50],
        danger: Color.hex("#fff0ee"),
        button: { primary: teal[400], secondary: ink[200], ghost: ink[50] },
    },
    text: {
        primary: ink[800], body: ink[700], secondary: ink[600],
        tertiary: ink[500], disabled: ink[400], // alpha applied at use site
        onAccent: ink[900], onDanger: Color.hex("#8a1a14"),
        placeholder: ink[400], code: ink[800], link: teal[800],
    },
    border: {
        subtle: ink[300], strong: ink[400], focus: teal[600],
    },
    accent: {
        primary: teal[400],   // for fills; use teal.800 for accent-as-text
        solid: teal[500], cursor: teal[500],
        complement: Color.hex("#ff8a72"),
    },
    status: {
        success: Color.hex("#00a06b"), warning: Color.hex("#c47700"),
        error: Color.hex("#d63a2f"),   info: teal[700],
    },
} as const;
```

Views import `tokens`, never primitives:

```ts
import { tokens } from "./tokens";
// ✓ Good
Container({ color: tokens.bg.panel, ... });
// ✗ Bad — primitive used directly
Container({ color: ink[100], ... });
// ✗ Bad — inline hex
Container({ color: Color.hex("#f4f6f9"), ... });
```

**Reactivity**: only wrap a token in `derive(() => ...)` when it depends on a source. Static tokens create the `Color` opaque once at module load — cheaper than rebuilding per frame.

---

## 9. Migration plan

Five phases. Each is independently shippable.

### Phase 1 — Token extraction (no UI change)
- Create `src/tokens.ts` per §8.
- Update `src/compile.ts` `KIND_HEX` to use `code.*` tokens.
- No visual change yet. PR: "chore: extract design tokens".

### Phase 2 — Replace inline hexes (visual change, no refactor)
- Walk `src/index.ts` against the table in §10, replacing each `Color.hex(...)` with the named token.
- No layout or view-shape changes — pure rename.
- PR: "refactor: replace inline hexes with design tokens".

### Phase 3 — Extract primitives (no visual change)
- Move `Sidebar` body → `NavList` + `NavItem`.
- Move `Editor` and `Viewer` headers → `Panel`.
- Move Run button → `Button({ variant: "primary", icon: PlayIcon })`.
- Move status label → `StatusBadge`.
- Move error overlay → `ErrorBanner`.
- One PR per primitive or one batched PR; reviewer's call.

### Phase 4 — Interactive states (visual improvement)
- Add `hover$` source per interactive view via `PointerInteract`'s `onPointerEnter`/`onPointerExit` (API already present).
- Apply the state matrix from §5.
- Add focus ring rendering (requires engine support for focus events — coordinate with platform team).

### Phase 5 — Iconography & motion polish
- Bundle Lucide SVGs as resources (§6).
- Add icons to Run button, ErrorBanner, optional NavItem selection indicator.
- Apply `motion.*` durations to interactive transitions (requires `createAnimationController` integration).

---

## 10. Migration table — every current inline hex

Map of every `Color.hex("...")` in the current `src/index.ts` and `src/compile.ts` to its replacement token. Phase 2 of the roadmap.

| File:line | Current hex | Current role | New token | Hex |
|---|---|---|---|---|
| `index.ts:127` | `#0f172a` | Placeholder bg | `bg.viewer` | `#fbfcfd` |
| `index.ts:133` | `#475569` | Placeholder text | `text.tertiary` | `#8a94a3` |
| `index.ts:147` | `#1e293b` | Selected item bg | `bg.selected` | `#7df5d0` |
| `index.ts:148` | `#0f172a` | Sidebar bg | `bg.panel` | `#f4f6f9` |
| `index.ts:156` | `#e2e8f0` | Selected item text | `text.primary` | `#1f2530` |
| `index.ts:157` | `#94a3b8` | Unselected item text | `text.body` | `#3a4250` |
| `index.ts:167` | `#0f172a` | Sidebar bg (outer) | `bg.panel` | `#f4f6f9` |
| `index.ts:177` | `#e2e8f0` | "tur playground" title | `text.primary` | `#1f2530` |
| `index.ts:203` | `#cdd6f4` | Editor text | `text.code` | `#1f2530` |
| `index.ts:204` | `#f5e0dc` | Editor cursor | `accent.cursor` | `#00a886` |
| `index.ts:205` | `#585b70` | Editor placeholder | `text.placeholder` | `#b8c0cc` |
| `index.ts:210` | `#1e1e2e` | Editor bg | `bg.code` | `#fbfcfd` |
| `index.ts:218` | `#11111b` | Editor header bg | `bg.header` | `#eceff4` |
| `index.ts:229` | `#94a3b8` | Editor header label | `text.secondary` | `#5e6878` |
| `index.ts:235` | `#313244` | Run button bg | `bg.button.primary` | `#00c69a` |
| `index.ts:240` | `#cdd6f4` | Run button text | `text.onAccent` | `#0a0e14` |
| `index.ts:275` | `#0b0b13` | Viewer bg | `bg.viewer` | `#fbfcfd` |
| `index.ts:282` | `#11111b` | Viewer header bg | `bg.header` | `#eceff4` |
| `index.ts:290` | `#94a3b8` | Viewer header label | `text.secondary` | `#5e6878` |
| `index.ts:300` | `#22c55e` | "ready" status | `status.success` | `#00a06b` |
| `index.ts:309` | `#ef4444` | "error" status | `status.error` | `#d63a2f` |
| `index.ts:334` | `#3b1116` | Error overlay bg | `bg.danger` | `#fff0ee` |
| `index.ts:339` | `#fca5a5` | Error overlay text | `text.onDanger` | `#8a1a14` |
| `index.ts:354` | `#0f172a` | Shell bg | `bg.app` | `#fbfcfd` |
| `compile.ts:19` | `#abb2bf` | code default | `code.fg` | `#1f2530` |
| `compile.ts:20` | `#c678dd` | code keyword | `code.keyword` | `#006e58` |
| `compile.ts:21` | `#98c379` | code string | `code.string` | `#3f7d3f` |
| `compile.ts:22` | `#d19a66` | code number | `code.number` | `#b35900` |
| `compile.ts:23` | `#7f848e` | code comment | `code.comment` | `#8a94a3` |
| `compile.ts:24` | `#56b6c2` | code operator | `code.operator` | `#5e6878` |
| `compile.ts:25` | `#e5c07b` | code literal | `code.literal` | `#92400e` |

---

## 11. Glossary

- **Primitive**: a raw palette entry (`teal.400`). Never used in views.
- **Semantic token**: a named role (`bg.button.primary`) mapped to a primitive. The view-facing layer.
- **View token**: an even more specific alias (`sidebar.item.bg.active`). Optional — only introduce when a view needs the same value across many files.
- **State matrix**: the standard set of visual states (default/hover/active/focus/disabled/selected) every interactive view must define.
- **AA / AAA**: WCAG 2.1 contrast levels. AA = 4.5:1 for body, 3:1 for large/UI. AAA = 7:1 / 4.5:1.
