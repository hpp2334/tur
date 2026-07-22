# tur Playground — Style Guide

**Companion to**: [`DESIGN-SYSTEM.md`](./DESIGN-SYSTEM.md) · **Scope**: `@tur/demo-impl`

The design system tells you *what* the tokens are. This guide tells you *how to think* about them — the principles behind product decisions, the patterns to reach for, and the mistakes to refuse in review.

---

## 1. Principles

Seven rules. Every visual decision should be defensible against at least one of them. When two principles conflict, the higher-numbered one wins.

### 1. Editor is the hero

The code editor is the thing the user came for. Everything else — sidebar, viewer, headers, status badges — exists to serve the editor and the running code.

**Practical implications**
- The editor surface gets the most pixel area (one of two equal `Expanded` panes, often the larger one in practice because sidebar is fixed-width).
- The editor's contrast is the highest in the app: `code.fg` `#1f2530` on `code.bg` `#fbfcfd` is 15.9:1.
- Never put a louder color in the chrome than in the editor. The Run button is `teal.400` — bright, but it's one element. The editor's keyword color (`teal.700` `#006e58`) is darker and more numerous; it carries weight without screaming.

**Smell test**: if a screenshot of the playground shows the chrome more prominently than the editor, something is wrong.

### 2. Calm by default, loud on signal

The default visual state is quiet: neutrals everywhere (`ink.50` → `ink.800`), low contrast between adjacent surfaces. Color appears only when there is something to communicate.

| Color | When it appears |
|---|---|
| `teal.400` | One persistent element: the primary CTA. Plus the editor cursor (a single line). |
| `teal.200` | One element: the currently-selected nav item. |
| `status.*` | Only when state is non-default (`error` ≠ `ready`). |
| `coral.400` | Never in the default state. Reserved for emphasis and onboarding. |

**Smell test**: take a screenshot, desaturate it. The page should still be readable. If desaturation removes information, you are using color as the only channel — see §7.5 in the design system.

### 3. Monospace for code, sans for UI

The visual seam between the editor and the surrounding chrome is the font switch. This is a deliberate signal: *this part is data, this part is interface*.

- `font.mono` (JetBrains Mono) → editor, inline code, keyboard hints (`⌘S`).
- `font.ui` (Inter) → everything else, including the Run button, status badges, error messages.

**Never** use monospace for body labels or sans for code. The only exception is the editor's `placeholderColor` text, which is monospace because it lives inside the editor.

### 4. Density over decoration

This is a developer tool. Users spend hours in it. Decorative chrome (heavy shadows, gradients, large paddings, illustrations) steals attention from the work.

- **Separation by background shift, not border**: panels differentiate by `bg.app` vs `bg.panel` vs `bg.elevated`. Reserve `border.subtle` for cases where the background shift alone is ambiguous (e.g. a header inside an already-elevated panel).
- **Shadows are rare and meaningful**: `elev.2` and above only for genuinely floating elements (modals, dropdowns). Never for a panel that sits in the layout grid.
- **Padding is the minimum to feel comfortable, not the maximum to feel spacious**. Default body padding is `space.md` (12), not `space.xl` (24).

**Smell test**: if removing a border or shadow makes the layout unreadable, the layout is wrong; the chrome was compensating.

### 5. Status before prose

When something is in a non-default state, communicate it with a symbol first and text second. Symbols are processed faster and survive quick glances.

- `<StatusBadge>` always renders a colored dot + a one-word label. Never just text, never just a dot.
- `<ErrorBanner>` always leads with `!` in a colored circle. The message text follows.
- The viewer header's status pill is `ready` or `error` — one word. Resist `ready (compiled at 12:34)`. Put the timestamp in a tooltip or secondary line.

**Corollary**: status colors are reserved. Do not use `status.success` green for "this is a leaf node" or `status.error` red for "this is required". Those aren't states, they're categories — use neutrals or `accent.complement` instead.

### 6. Tokens, not hexes

No `Color.hex("...")` outside `src/tokens.ts` and `src/compile.ts`'s `code.*` definitions. Every visual value traces back to a named token.

This rule has three payoffs:
1. **Rethemable**: a future dark mode (or branded variant) only edits `tokens.ts`.
2. **Reviewable**: a PR that introduces `Color.hex("#...")` in a view is automatically suspect — the reviewer doesn't have to read every line.
3. **Discoverable**: when a designer asks "what's our success green?", there's one answer.

See §4 below for the biome lint rule that enforces this.

### 7. Static unless reactive

Only wrap a value in `derive(() => ...)` when it actually depends on a source (`source()`, another `derive()`, or a controller state). Static values cost less: they're computed once at module load and reused.

```tsx
// ✗ Bad — `derive` for a value that never changes
color: derive(() => tokens.bg.panel)

// ✓ Good — static
color: tokens.bg.panel

// ✓ Good — reactive (depends on selectedCase$)
color: derive(() => get(selectedCase$) === name
    ? tokens.bg.selected
    : tokens.bg.panel)
```

This rule exists for two reasons: (a) `derive` adds a subscription and recomputes on every relevant change, even if the function body produces the same value; (b) it makes the code read worse — `derive` should signal "this value changes", and using it for statics trains reviewers to ignore it.

---

## 2. Voice & tone

### 2.1 Labels

- **Sentence case** everywhere: `Ready`, not `READY` or `ready`. `Run`, not `RUN`.
- **Verbs for actions**, nouns for states: `Run` (action), `ready` (state). The Run button is a verb; the status badge is a state.
- **Short and specific**: `editor — counter`, not `Editor — Currently viewing case: counter`.
- **No exclamation marks.** No marketing tone. The playground is a tool.

### 2.2 Empty states

Empty states are honest about why they're empty and what to do next.

| Context | Bad | Good |
|---|---|---|
| No case selected | `Empty` | `(no case)` — matches the current `Placeholder`. Future: `Select a case from the sidebar.` |
| Compile error | `Error` | `Unexpected token at line 12:4` — the actual error from the compiler, with location. |
| Runtime error | `Something went wrong` | `Cannot read property 'foo' of undefined` — the actual message. |

**Never** invent a friendly paraphrase of an error. Developers can read real errors; paraphrases lose information.

### 2.3 Hints

Hints teach the user a shortcut, then disappear.

- `Run (Cmd-S)` on the Run button is correct: it labels the action and teaches the shortcut in one move. Once the user knows `Cmd-S`, the label still makes sense — it doesn't become noise.
- Avoid tooltip-only hints. If a hint matters, put it inline until the user demonstrates they don't need it.

### 2.4 Numbers and units

- Pixel values are written without units in code (`fontSize: 13`) and with `px` in docs (`13px`).
- Durations always include the unit (`120ms`, never `120`).
- Percentages for opacity, never for layout (use `Expanded`).

---

## 3. Patterns

### 3.1 When to use `PointerInteract` vs `Switch` vs `Condition`

| Need | Use | Because |
|---|---|---|
| A click handler on a single element | `PointerInteract` | Adds hover/active/focus paths. Default for buttons, nav items. |
| Swap entire subtrees based on a source | `Switch` | Atomic swap — old subtree is disposed, new one is created. The right tool when the shape of the subtree changes (e.g. different case → different view factory). |
| Show/hide a single subtree | `Condition` | Cheaper than `Switch` — no key matching, just a boolean gate. Use for overlays, error banners. |
| Map an array to elements | `Each` | Identity-preserving reconciliation. Never use `.map()` inside `children` for arrays that change. |

**Smell test**: if you're writing `Switch({ cases: [{key:"true",...},{key:"false",...}] })`, you mean `Condition`. If you're writing `Condition({ child: A(), elseChild: B() })` where `A` and `B` have completely different shapes driven by a `selectedX$`, you mean `Switch`.

### 3.2 Reactive color pattern

Interactive elements with hover/selected states follow this template:

```tsx
const hover$ = source(false);
// ...
PointerInteract({
    onClick,
    onPointerEnter: mutate(() => set(hover$, true)),
    onPointerExit: mutate(() => set(hover$, false)),
    child: Container({
        color: derive(() => {
            if (get(selected$)) return get(hover$) ? teal300 : teal200;
            return get(hover$) ? hoverColor : defaultColor;
        }),
        // ...
    }),
})
```

Two sources, one `derive` that returns a token. Don't nest `derive` inside `derive`; don't compute color strings — return token references directly.

### 3.3 Controller lifecycle

Controllers (`createTextEditingController`, `createScrollController`, ...) are created at module scope and live for the lifetime of the page. They are **not** recreated per render.

- ✓ Controller created once, `controller` prop passed down.
- ✗ Controller created inside `view(() => ...)` — it'll be recreated when the view rebuilds, losing state.

The current `editorCtrl` in `index.ts` is correct: created at module scope, used by every render.

### 3.4 Naming

- **Sources** end in `$`: `selectedCase$`, `status$`, `errorMsg$`, `hover$`. Convention from the reactive-programming world; makes data flow obvious in code review.
- **Views** are PascalCase, one word where possible: `Sidebar`, `Panel`, `Button`. Multi-word: `EditorSurface`, `NavItem`, `StatusBadge`.
- **Token groups** are lowercase nouns: `tokens.bg.panel`, `tokens.text.primary`, `tokens.status.success`.
- **Mutations** (event handlers) begin with a verb in the present tense: `recompile`, `loadCase`. Not `onRecmpile`, not `handleRecompile`.

### 3.5 File structure

```
src/
  tokens.ts              # The token layer (only place Color.hex is allowed)
  compile.ts             # Case compiler + code.* syntax highlighting
  index.ts               # App entry: sources, controllers, render(Shell)
  views/
    Shell.ts
    Panel.ts
    NavItem.ts
    NavList.ts
    Button.ts
    StatusBadge.ts
    EditorSurface.ts
    ViewerSurface.ts
    ErrorBanner.ts
    Placeholder.ts
  icons/
    resources.ts         # createImageResource calls for bundled SVGs
```

Each view is one file, one default export, no side effects at module load.

---

## 4. Code conventions

### 4.1 Import order

```ts
// 1. tur:std primitives
import { Color, Column, Container, /* ... */ } from "tur:std";
// 2. Local utilities
import { tokens } from "./tokens";
import { Button } from "./views/Button";
// 3. Types (type-only import)
import type { Element } from "tur:std";
```

### 4.2 The no-hex lint rule

Add this to `biome.json` under the playground package (phase 1 of the roadmap):

```json
{
    "lint": {
        "noRestrictedSyntax": {
            "options": [
                {
                    "selector": "CallExpression[callee.object.property.name='Color'][callee.property.name='hex']",
                    "message": "Use tokens from src/tokens.ts instead of Color.hex(). Exception: tokens.ts itself."
                }
            ]
        }
    }
}
```

This makes any new `Color.hex("...")` outside `tokens.ts` a lint error. The token extraction in phase 1 is the prerequisite — the rule turns on once all current hexes are migrated.

### 4.3 View file template

```ts
// src/views/Button.ts
import {
    type Element, Container, PointerInteract, Row, Text,
    derive, get, mutate, set, source,
} from "tur:std";
import { tokens } from "../tokens";

export interface ButtonProps {
    variant?: "primary" | "secondary" | "ghost";
    label: Parameters<typeof Text>[0]["text"];
    onClick?: Parameters<typeof PointerInteract>[0]["onClick"];
    disabled?: boolean;
}

export function Button(props: ButtonProps): Element {
    const variant = props.variant ?? "secondary";
    const hover$ = source(false);
    const pressed$ = source(false);

    return PointerInteract({
        onClick: props.onClick,
        onPointerEnter: mutate(() => set(hover$, true)),
        onPointerExit: mutate(() => { set(hover$, false); set(pressed$, false); }),
        child: Container({
            padding: 8,
            borderRadius: 6,
            color: derive(() => /* variant/hover/pressed matrix */ tokens.bg.button.secondary),
            children: [Row({ children: [
                Text({ text: props.label, fontSize: 13, color: tokens.text.primary }),
            ]})],
        }),
    });
}
```

Every view file follows this shape:
1. Type-only imports first, value imports second, `tokens` third.
2. `Props` interface, exported.
3. Single factory function, exported.
4. Sources for local UI state (`hover$`, `pressed$`) declared at function scope — they reset on rebuild, which is what you want.

---

## 5. Do / Don't

### 5.1 Color

**Do** — Use semantic tokens:
```tsx
Container({ color: tokens.bg.panel })
Text({ color: tokens.text.secondary })
```

**Don't** — Inline hexes:
```tsx
Container({ color: Color.hex("#f4f6f9") })  // Where does this come from? Why this value?
Text({ color: Color.hex("#5e6878") })
```

**Don't** — Use primitives directly in views:
```tsx
Container({ color: ink[100] })  // Views shouldn't know about the ink scale
```

### 5.2 Reactivity

**Do** — Static where static, reactive where reactive:
```tsx
// header that never changes
Container({ color: tokens.bg.header })

// item that highlights when selected
Container({ color: derive(() => get(selected$) ? tokens.bg.selected : tokens.bg.panel) })
```

**Don't** — Wrap statics in `derive`:
```tsx
Container({ color: derive(() => tokens.bg.header) })  // useless subscription
```

**Don't** — Nest `derive`:
```tsx
// ✗ Bad
const a$ = derive(() => get(x$) ? teal400 : ink300);
const b$ = derive(() => get(y$) ? a$ : somethingElse);  // reads a derived inside another derive

// ✓ Good — combine into one
const color$ = derive(() => {
    if (!get(y$)) return somethingElse;
    return get(x$) ? teal400 : ink300;
});
```

### 5.3 View composition

**Do** — Compose via `Panel`:
```tsx
function ViewerSurface(props): Element {
    return Panel({
        title: "viewer",
        actions: [StatusBadge({ status: props.status$ })],
        body: props.child$,
    });
}
```

**Don't** — Hand-roll headers per panel:
```tsx
// ✗ Sidebar, Editor, Viewer each repeat this 12-line block
Container({ padding: 8, color: /* header color */, children: [Row({ ... })] })
```

### 5.4 Status

**Do** — Status badge with both dot and label:
```tsx
StatusBadge({ status: "ready" })  // renders • ready
```

**Don't** — Color-only status:
```tsx
// ✗ Just a green dot, no label
Container({ width: 6, height: 6, color: tokens.status.success, borderRadius: 999 })

// ✗ Just colored text, no dot
Text({ text: "ready", color: tokens.status.success })
```

### 5.5 Spacing

**Do** — Use the scale:
```tsx
Container({ padding: 12 })  // space.md — easy to scan
```

**Don't** — Off-grid values:
```tsx
Container({ padding: 10 })  // off-scale, will confuse reviewers
Container({ padding: 13 })  // ditto
```

**Exception**: `padding: 6` is permitted for very tight inline cases (chips with a leading icon), since the next step up (`space.sm` = 8) is sometimes too loose. Document the exception in a comment.

### 5.6 Editor configuration

**Do** — Use the canonical editor config from `<EditorSurface>`:
```tsx
Input({
    controller,
    multiline: true,
    fontFamily: "mono",
    fontSize: 13,
    color: tokens.text.code,
    cursorColor: tokens.accent.cursor,
    placeholderColor: tokens.text.placeholder,
})
```

**Don't** — Custom per-instance editor configs:
```tsx
// ✗ Some other place renders an Input with different sizes/colors
Input({ controller, fontSize: 14, color: Color.hex("#333") })
```

There is exactly one editor surface in the playground. If a future feature needs an inline editable text (e.g. renaming a case), that's a different view (`<TextInput>`), not a second `<EditorSurface>`.

---

## 6. PR review checklist

Before approving any PR that touches `@tur/demo-impl`, verify:

- [ ] **No new `Color.hex(...)` outside `tokens.ts` / `compile.ts`.** Run `rg 'Color\.hex' js/packages/tur-demo-impl/src` and confirm the only matches are in those two files.
- [ ] **No primitive tokens used directly in views.** `rg 'ink\.\d|teal\.\d|coral\.\d' js/packages/tur-demo-impl/src/views` should return nothing.
- [ ] **No `derive(() => ...)` wrapping static values.** Grep for `derive` and check each one reads at least one source via `get(...)`.
- [ ] **No off-scale spacing.** Search for `padding:`, `margin:` (if introduced), and verify values are in `{0, 4, 8, 12, 16, 20, 24, 32, 40, 48, 64}`. Documented exceptions allowed with a comment.
- [ ] **No off-scale font sizes.** Values must be in `{10, 11, 13, 14, 18}` for UI, `13` for code.
- [ ] **No new font families.** Only `mono` and the default UI family.
- [ ] **Interactive elements have all five states.** Default, hover, active, focus, disabled — even if some are no-ops, they must be considered. See design system §5.
- [ ] **Status badges have both dot and label.** No color-only signaling.
- [ ] **Error messages are the real error, not a paraphrase.**
- [ ] **Labels are sentence case.** `Run`, `ready`, `editor — counter`. Not `RUN`, not `Ready`.
- [ ] **View files follow §4.3 template.** Imports ordered, `Props` exported, one factory per file.
- [ ] **No decorative shadows on layout-bound panels.** `elev.2` and above only for genuinely floating UI.
- [ ] **If adding a new view**, it appears in `DESIGN-SYSTEM.md` §4 with an API table and state matrix.
- [ ] **If adding a new token**, it appears in `DESIGN-SYSTEM.md` §2 with its primitive mapping and use case.

---

## 7. FAQ

**Q: Can I add a new color for a one-off case (e.g. a special highlight)?**
A: No. Either it's a new semantic token (add it to `tokens.ts` and the design system) or it's an existing token. One-off colors are how the original 17-hex chaos happened.

**Q: The viewer's case content uses bright colors (rainbow test case). Does that violate "calm by default"?**
A: No. The principle governs the *chrome* — the playground shell. Case content is the user's data, rendered by their view. The shell stays calm so the user's content can be loud.

**Q: What about a future dark mode?**
A: The token layer is designed for it: add a parallel `tokens.dark` object and a theme switch. Views stay unchanged. Phase 5+ — not in scope for the current light-theme migration.

**Q: Why is `bg.viewer` the same color as `bg.code` (`ink.50`)? Why not differentiate?**
A: The viewer and editor are sibling surfaces with equal weight (both Expanded). Differentiating their backgrounds would imply one is "above" the other. Use the same color; let the content differentiate them.

**Q: Can I use `coral` for warnings?**
A: No. `coral` is the brand's warm complement, used for emphasis and onboarding. Warnings use `status.warning` (`#c47700`, amber). Coral and warning-amber are different colors with different jobs.

**Q: The Run button is currently a grey box (`#313244`). Why change it to bright teal?**
A: The Run button is the *only* persistent action in the playground — it's how you recompile after editing. Making it the primary CTA (`bg.button.primary` = `teal.400`) signals its importance. The previous grey made it visually equivalent to the surrounding chrome, which understated its role.

---

## 8. Escalation

When this guide doesn't have an answer:

1. **Check the design system first** — it has more tables and less prose.
2. **Check existing views** — find the closest analog and copy its pattern.
3. **Make the smallest defensible choice** and flag it in the PR description as needing design review. Don't block on it; ship and iterate.
4. **Update this guide** once the decision is made. The guide grows; the chaos doesn't.

---

## 9. Addendum — toolbar & status-bar patterns (post-redesign)

### 9.1 New principle (priority 8): Status bar shows truth

The status bar at the bottom of the window is the single source of truth for "what state am I in?". Anything that qualifies as runtime state — selected case, edited-ness, last-compiled timestamp, current error — must be answerable by glancing at the status bar.

**Practical implications**
- Never show state in only one place. If `status$ === "error"`, it appears as: status dot + label in status bar, AND the viewer is replaced by the ErrorPanel. The redundancy is intentional.
- The status bar updates on every relevant state change. Use a `launch` loop (`launch(function*(){ for(;;){ yield sleep(5000); set(now$, Date.now()); } })`) driving a `now$` source for time-relative fields ("compiled Xs ago") — don't try to invalidate strings manually.
- The status bar is ~24px tall. Treat it as scannable, not readable: short labels, single dot indicators, no full sentences.

When this principle conflicts with principle 1 (Editor is the hero), the editor's needs win for the body area, but the status bar is sacrosanct chrome — never hide it to give the editor more space.

### 9.2 Pattern: shared hover sources

Every interactive group (sidebar list, toolbar button cluster, segmented control) uses **one shared `hovered$` source per group**, not one source per item. This keeps the subscription graph flat and makes "what's hovered" a query rather than N independent booleans.

```ts
// ✓ Good — one source, items compare by key
const hoveredCase$ = source<string | null>(null);
// In each item's PointerInteract:
onPointerEnter: mutate(() => set(hoveredCase$, name)),
onPointerExit: mutate(() => set(hoveredCase$, null)),
// In each item's Container color derive:
color: derive(() => get(hoveredCase$) === name ? hoverColor : defaultColor),

// ✗ Bad — one source per item (N subscriptions, N wasted re-renders)
const hover$ = source(false);  // per NavItem closure
```

For lone interactive elements (RunButton, ResetButton), a single boolean source is fine — the group has one member.

### 9.3 Pattern: layout via flex `derive`, not `Switch`

When only the **flex ratio** between siblings changes (no structural change), bind `Expanded.flex` to a `derive()` rather than rebuilding the tree via `Switch`:

```ts
// ✓ Good — flex bound to source; editor cursor and viewer state survive mode change
Expanded({
    flex: derive(() => layoutFlex("editor", get(layoutMode$))),
    child: Editor(),
}),
Expanded({
    flex: derive(() => layoutFlex("viewer", get(layoutMode$))),
    child: Viewer(),
}),

// ✗ Worse — Switch rebuilds both panes on every mode change, losing editor state
Switch({
    value: layoutMode$,
    cases: [
        { key: "split",   child: Row({ children: [Expanded({child: Editor()}), Expanded({child: Viewer()})] }) },
        { key: "editor",  child: Row({ children: [Expanded({flex:2, child: Editor()}), Expanded({child: Viewer()})] }) },
        ...
    ],
})
```

Reserve `Switch` for cases where the **shape** of the subtree genuinely changes (different element types, different queryKeys, different children).

### 9.4 Pattern: every `Row` inside a Container needs `mainAxisSize: Min`

This is the most common layout bug in the redesign. The engine's `Row` defaults to `MainAxisSize::Max`, which means "expand to fill parent". When a Row is the child of an unset-width Container inside another Row, the inner Row will try to consume the outer Row's full available width — pushing siblings off-screen.

**Rule**: any `Row` that is the direct child of a `Container` (without an explicit width) MUST set `mainAxisSize: MainAxisSize.Min` unless it is genuinely meant to fill the container's width.

```ts
// ✓ Buttons, badges, labels — content-sized
Container({
    padding: 6,
    borderRadius: 6,
    color: tokens.bg.button.primary,
    children: [Row({
        mainAxisSize: MainAxisSize.Min,   // ← critical
        children: [
            Text({ text: "▶", ... }),
            SizedBox({ width: 4 }),
            Text({ text: "Run", ... }),
        ],
    })],
}),

// ✓ Layout row that should fill — no mainAxisSize needed
Row({
    children: [
        Brand(),                    // content-sized (its inner Row is Min)
        Expanded({ child: ... }),   // fills remaining
        Actions(),                  // content-sized
    ],
})
```

**Symptom of violation**: a Container reports its width as the parent's full width even though its visible content is much narrower. Debug via `JSON.parse(globalThis.turDevTool.elementTree())`.

### 9.5 Pattern: debounced auto-run via `launch` + `sleep`

Auto-run debouncing uses the engine's `sleep` + `launch` task primitives (see `libs/tur-engine/src/core/bridge/task.rs`). `launch` runs a generator function as a cancellable coroutine; `yield sleep(ms)` suspends it. Each keystroke cancels the pending task and launches a fresh one — only the last survives:

```ts
import { launch, mutate, sleep, type Task } from "tur:std";

let autoRunTask: Task | null = null;

const editorCtrl = createTextEditingController({
    onInput: mutate(() => {
        // ... sync work like syntax highlighting ...
        if (get(autoRun$)) {
            autoRunTask?.cancel();
            autoRunTask = launch(function* () {
                yield sleep(300);
                recompile();
            });
        }
    }),
});
```

- `launch` returns a `Task`; call `task.cancel()` to supersede it (the generator body after the current `yield` never runs again). The in-flight `sleep` resolves harmlessly and is ignored.
- Set the local `autoRunTask` to `null` inside the callback isn't needed — the cancelled task is simply abandoned; reassign on the next schedule.
- Also cancel inside `recompile()` itself (in case the user hits Cmd-S while a debounce is pending).

For periodic timers (e.g. a countdown), use a `launch` loop with a `running$` flag check rather than a cancel handle: `launch(function* () { while (get(running$)) { yield sleep(1000); … } })`.

`launch` accepts any Promise on the right of `yield`, not just `sleep`. A rejected yielded promise throws its reason at the `yield` point — wrap the `yield` in `try/catch` to handle failures (the same ergonomics as `await`). Prefer this linear generator form over nested `.then(...).catch(...)` chains for sequential async work (e.g. HTTP fetches):

```ts
launch(function* () {
    try {
        const r = (yield http({ method: "GET", url, responseType: "text" })) as HttpResponse;
        // ... use r ...
    } catch (e) {
        set(error$, errMsg(e));
    } finally {
        set(loading$, false);
    }
});
```

### 9.6 Pattern: full-state-replacement for errors

When the application enters a non-default state (error, loading, empty), **replace the relevant pane's body entirely** rather than overlaying a banner. This forces the user to address the state before continuing.

```ts
function Viewer(): Element {
    return Switch({
        value: status$,
        cases: [
            { key: "ready", child: ReadyViewer() },   // normal render
            { key: "error", child: ErrorPanel() },    // full-panel replacement
        ],
        fallback: ReadyViewer(),
    });
}
```

**Do not** show a small banner at the bottom of an otherwise-normal pane for important states. Banners are easy to miss; users assume the rendered content is still valid.

**Exception**: transient, non-blocking notices (e.g. "saved") can be brief overlays. Errors are never transient.

### 9.7 Review checklist additions

Add to §6's PR review checklist:

- [ ] Every `Row` inside a `Container` without explicit width sets `mainAxisSize: MainAxisSize.Min` (unless it's meant to fill).
- [ ] Every interactive group uses one shared `hovered$` source, not per-item sources.
- [ ] Layout changes that only affect flex ratios use `derive()` on `Expanded.flex`, not `Switch`.
- [ ] Auto-run / debounce paths supersede the previous `Task` via `cancel()` before launching a new one (`launch` + `yield sleep`).
- [ ] Sequential async (HTTP fetches, clipboard reads) uses `launch` + `yield` with `try/catch` for rejections, not nested `.then(...).catch(...)` chains.
- [ ] State appears in at least two places (e.g. error dot in status bar + ErrorPanel in viewer).
- [ ] Status bar contents are scannable (single dot + 1-2 word labels), not prose.
