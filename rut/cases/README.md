# The shared case corpus (rut)

The case corpus drives three consumers:

1. **Rust integration tests** — `TurTestApp::load_bundle` / `load_rut_bundle`
   load a case by name (`<name>/index.rut` for the rut rail; the
   legacy `dist/<name>.js` path is the JS rail's, retired with Phase 4).
2. **The playground** — `rut/playground/scripts/gen-cases.cjs` embeds
   the case sources verbatim so the sidebar can list + open every case.
3. **Rut-semantics documentation** — the cases are the reference examples
   for authoring tur UIs in rut (see the `tur_kit` prelude —
   `rut/tur_kit/tur_kit.rut` — for the builder classes each
   example uses, and each builtin plugin's `rut_rows.rs` for the row
   surface underneath).

## The per-case contract (rut)

Every case is a **single `index.rut` module** with:

- **`entry fn start()`** — the ONLY mount point. It authors the tree
  through the **kit** builder classes (`use tur_kit::{ Column, Text, … };`
  — constructed by class call (`Column()`, `Text()`; the `.builder()` long
  form stays valid — byte-identical lowering), chainable one method per
  prop, CHAINED — never `let mut x` + `x.prop(..)` statement runs — with
  `.child(c)` / `.children([…])` appending and `.build()` the only
  terminal) and hands the root to the engine
  with `mount(root.build())`. `start` may return `-> u64` (the host records
  the answer — conventionally the id of the module's root state atom —
  readable via `TurTestApp::rut_start_answer`).
- **probe `entry fn`s** — named entries the *test* drives via
  `app.call_rut_entry(name, a, b)` (the engine→rut event rail). Probes
  replace the JS era's `eval_js` state pokes. `entry` = a deliberate
  embedder/test contract — element callbacks are NOT entries.
- **no JS anywhere** — a rut case never touches a JS realm. State lives
  in source atoms minted through the kit (`source_f64` / `source_str` /
  `source_bool` / `source_value` — the `rs_*` rows are the substrate case
  code never calls) whose ids cross into entries as plain `u64`s. Tests
  read state back through **dev-tool tree queries** (`query_element`,
  `dev_tool_element_tree`, `query_text`) or bound atoms — never by
  evaluating script.

### Conventions

- **Naming**: the directory name is the case name (`kebab-case`), e.g.
  `cases/counter/index.rut` loads as `"counter"`.
- **Query keys**: give every element a test needs to find a query key via
  the element's chainable `.query_key("key")` method; tests locate it with
  `app.query_element(&["key"])`.
- **Handlers are mutations** — `on_click(mutate(fn (ctx: MutationCtx) {
  … }))` over the boa triad `source_*` / `derive_*` / `mutate` (round 5).
  State rides by capture, writes flow through `ctx.set_*`, reads through
  `ctx.get_*`, composition through `ctx.run*`; the flush's mutation pass
  invokes them — never synchronously inside the dispatch. `on_tap`, the
  `2` twins and `.ids()`/`.id()` are GONE. The remaining fn values are
  the SUBSTRATE: view-builder callbacks stay plain `fn`s (conventionally
  prefixed by their role — `Each(items).item_builder(fn (i: u64, item:
  str) -> View { … })`), sealed into opaque boxes at the kit boundary
  (compile-time arity/type-checked) and fired through the infra dispatch
  entries. See the round-5 section below for the full architecture note.
- **Determinism**: cases run under the harness's virtual clock. Time-based
  behavior (tickers, animations) rides `anim_ctrl` durations so tests can
  advance time deterministically.
- **JS-semantics cases have no rut twin** — module-loader behavior,
  promise-rejection reporting and JS error shapes were JS-rail concerns;
  the rut rail's equivalents (traps, fuel, parse diagnostics) are pinned by
  `libs/tur-integration-tests/tests/rut_boot.rs` instead.
- **`compiler-bridge-demo` is intentionally omitted** — it demoed the JS
  transpile bridge, which no longer exists in the rut era.

### Content parity vs the boa reference (phase-8 audit)

Side-by-side sweep of the curated 19 (rut 8080 vs the boa dist 8081; the
editor language differs by design — this grades the VIEWER: content
presence, layout spirit, interactivity affordances). Ranked, thinnest
first — the follow-up queue (drained across rounds 3–4; the round-3
ledger below is the final state):

| case | parity | gap |
|---|---|---|
| `complex-animation` | ✅ upgraded | was a bare opacity tween; now the "Animated Card Studio" (controller transport, speed/curve/loop selectors, status badge + % readout, width + hue + opacity channels) — motion verified natively AND in the wasm viewer side-by-side. Remaining vs boa: static card radius (no bound-radius row), no orbiting dot (no trig rows in the engine VM's offer set), Transform rotation static (the `el_transform` row takes no atom). |
| `jigsaw-puzzle` | ⚠️ thin | boa is a full 3×3 board game (shuffle, placed counter, scramble); rut is the one-tile drag fixture (by design — the corpus regression fixture family). |
| `grid-gallery` | ⚠️ thin | boa: interactive gallery (aspect + density chips, selection ring); rut: three static dark tiles. |
| `grid-aspect` | ⚠️ thin | boa: 10-tile HSL grid over `childAspectRatio: 2`; rut: three fixed 100×100 tiles in a Row (no Grid). |
| `grid-basic` | ⚠️ thin | boa: 12-cell derived-count grid; rut: 2×2 static. |
| `implicit-animations` | ⚠️ moderate | mechanism parity (shared controller + retarget) but the rut box binds nothing visible — the tick's progress atom drives no prop; boa shows an animated Compact/Expand card. |
| `lazy-list-virtualized` | ⚠️ moderate | both scroll 2000+ virtualized rows; rut rows are near-invisible (dark text on dark cells), boa is a contact list with avatars. |
| `lazy-list-var-sizes` | ⚠️ moderate | var-extent rows work; boa adds the axis-flip toggle and readable bar-chart rows. |
| `lazy-grid-basic` / `-gallery` / `-scroll` | ⚠️ moderate | LazyGrid + header/chips present; rut cells are unlabeled dark boxes vs boa's labeled hue tiles. |
| `todolist` | ✅ ok | per-item toggle, add, derived count all work; boa adds task cards, descriptions, a remove modal. |
| `countdown` | ✅ ok | ticking, start/pause/reset, edit modal all work; boa chrome is richer (hero digits, status pill, button hierarchy). |
| `counter` | ✅ fixed | content parity; the top-anchored column now centers (`Alignment.center` over an Expanded fill — #26). |
| `github-viewer` | ✅ ok | single-file port: landing + chips + error banner + explorer; missing the prefilled input and the repo banner. |
| `password-input` | ✅ ok | obscure toggle parity; boa adds the title + plain-echo input. |
| `table-basic` | ✅ ok | same planets/columns; rut renders the dark frame vs boa's light theme. |
| `table-reactive` | ✅ ok | comparable reactive emphasis (rut: Each-driven add-row; boa: async fetch + sortable headers). |
| `text-demo` | ✅ full | section-for-section parity (size/weight/color/spans/overflow + the maxLines cycler). |

The recurring rut-side tells: (1) dark-on-dark cell palettes read as
broken — pick readable fills; (2) the grid cases predate the Grid family
(port them over `Grid`/`LazyGrid`); (3) chips/toggle chrome is cheap in
the kit now — the boa cases lean on it heavily.

### Example

```rut
use tur_host::{ ALIGN_CENTER, MAIN_ALIGN_CENTER, mount };
use tur_kit::{ Column, Container, DeriveCtx, Mutation, MutationCtx, PointerInteract, Readable,
    Text, derive_str, mutate, source_f64 };

entry fn start() -> u64 {
    let count: Readable<f64> = source_f64(0.0);
    let label: Readable<str> = derive_str(fn (ctx: DeriveCtx) -> str {
        return f"Count: {ctx.get_f64(count) as u64}";
    });

    // The named mutations (boa's named `mutate(...)` values); the source
    // rides by capture — no ids, no stash.
    let b_inc = mutate(fn (ctx: MutationCtx) {
        ctx.set_f64(count, ctx.get_f64(count) + 1.0);
    });

    let col = Column()
        .main_alignment(MAIN_ALIGN_CENTER)
        .query_key("col")
        .child(Text().text_bound(label).query_key("count").font_size(36.0).build())
        .child(button(b_inc, "+1", "inc"));
    mount(col.build());
    return count.atom_id();
}

// A pill button: a PointerInteract pad taking the MUTATION. Handlers are
// mutations; view-builder callbacks (item_builder & co) stay plain fns.
fn button(b: Mutation<nil, nil>, text: str, key: str) -> opaque {
    return PointerInteract()
        .on_click(b)
        .child(
            Container()
                .color(0x6366F1FFu64)
                .query_key(key)
                .child(Text().text(text).build())
                .build(),
        )
        .build();
}
```

The test side:

```rust
let mut app = TurTestApp::new(400.0, 600.0).unwrap();
app.load_bundle("counter").unwrap();
app.wait_for_timeout(Duration::ZERO);
assert_eq!(app.query_text(&["count"]).as_deref(), Some("Count: 0"));

// Drive the on_click pad: locate it by query key, click its center.
let pi = ElementNodeId::new(app.query_element(&["inc"]).unwrap().as_u64());
let (cx, cy) = app.get_element_absolute_bounds(pi).unwrap().center();
app.click(cx, cy);
app.wait_for_timeout(Duration::ZERO);
assert_eq!(app.query_text(&["count"]).as_deref(), Some("Count: 1"));
```

## Case-parity audit + final re-audit (round 3 — the playground-case-parity plan)

The operator audit of 2026-10-06 (19 showcase cases × both playgrounds, 40+
screenshots) started the `playground-case-parity` plan: **5 MINOR, 13 MAJOR +
1 both-sides-broken** (table-reactive). The bar: match boa's DESIGNS, not
boa's bugs (Appendix A). Phases 0–9 landed the engine gaps and the case
rewrites (b17d898 callback rail, a1e5545 grid ceil, 3977a3e va-child
keys/IME, 7e58be8 wheel routing, eba6760 hit-test verdict, ae02e36 flex
sizing verdict, 0c35ff7 trig + lazy branches + obscure/radius/pos bound
rows, 1e8f9fe + 83c27a3 case rewrites, 8158cc9 jigsaw game). The table is
the final ledger: each case's original verdict, what changed, and the
phase-10 re-audit status (fresh wasm byte-verified vs dist; same method as
the original — select each case in both playgrounds, screenshot, probe the
interactive affordances).

| # | case | original verdict | what changed (phases) | re-audit status |
|---|------|------------------|-----------------------|-----------------|
| 1 | complex-animation | MINOR — no orbit dot (no trig rows), no rotation channel, control rows hugged left | `math_sin`/`math_cos` + `radius_bound` rows (ph6); rows centered, rotation restored via the rebuild channel (a one-item Each re-mounts the square under a fresh Transform — `el_transform` takes static angles only), orbiting dot on bound Positioned anchors, radius animates 8→40 (ph8); rotation re-riden onto the bound-angle row — `Transform().rotate_bound(progress)`, the rebuild channel dropped (round-4 ph2) | ✅ PASS — dot orbits, square rotates, width/hue/% animate; round-4: `elementTree()` byte-stable mid-play, zero per-tick remount |
| 2 | countdown | MAJOR — ~16px display, four flat pills, top-aligned, no status chip | full boa styling: 96px display + COUNTDOWN eyebrow, centered pane, ● Ready/Running pill, big Start⇄Pause Condition swap; edit modal via `then_build` (fresh controller prefilled with the live `initial` — the re-seed workaround dropped); mid-tick-edit clock-freeze fix + regression test (ph7) | ✅ PASS — full flow incl. edit modal + reset |
| 3 | counter | MINOR — button row hugged the left edge (expanding `gap()`) | gap dropped, `MAIN_ALIGN_CENTER` + page centering; count readout rides a derive (ph8) | ✅ PASS — row centered |
| 4 | github-viewer | MINOR — "GH" text badge, no Repository label, no prefill, off chip set | octocat SVG badge (`img_res_svg`), "Repository" label, prefill "facebook/react", boa chip set facebook/react · tailwindlabs/tailwindcss · vuejs/core (ph8) | ✅ PASS — Browse renders the live explorer |
| 5 | grid-aspect | MAJOR — three solid tiles, no Grid | grid ceil semantics (ph1); full port: `Grid().max_cross(160).aspect(2.0)` of 10 labeled hue cells + a second grid exercising `.main_extent(40)` (ph7) | ✅ PASS — both grids match |
| 6 | grid-basic | MAJOR — 2×2 static slab | ceil semantics (ph1); 12 hue-ramp cells on `Grid().max_cross(140).spacing(8,8)` (ph7) | ✅ PASS — 12 labeled cells; rut's 4 columns vs boa's 3 is the sanctioned Flutter ceil (boa floors) |
| 7 | grid-gallery | MAJOR — three dark tiles; no header/chips/derives | ceil semantics (ph1); header band via `CROSS_ALIGN_STRETCH` (ph5); full port: live derive subtitle ("tile #i · 1:1 · maxExtent 150"), aspect + density chips, scrollable 27-tile grid, tap-to-select rings; chips re-key one flat Switch over the combined mode string (deliberately avoiding the nested-switch wall) (ph7) | ✅ PASS — 16:9 reshapes + subtitle; Dense re-keys 3→5 columns; ring follows taps |
| 8 | implicit-animations | MAJOR — stub: empty `a_tick`, static box, tap did nothing | wired end-to-end: 600ms easeInOut controller drives width 100→200 / height 40→80 / color sky→indigo / radius 12→40 (`radius_bound`) / slide / label fade; Compact↔Expanded via Condition (ph8) | ✅ PASS — mid-flight + settled shots |
| 9 | jigsaw-puzzle | MAJOR — one dead fixture piece (double-build authoring; drag dead) | single-build authoring + bound label; hit-test verdict: case-authoring, engine sound (ph4); full 3×3 game: 9 pieces on bound anchors, snap+lock, "N / 9 placed" derive badge, Shuffle (Park–Miller LCG), Solved! banner; native pins drive the full loop (ph9) | ✅ PASS — snap highlight, lock, 1/9; wrong drop refused; Shuffle re-deals |
| 10 | lazy-grid-basic | MAJOR — floor columns (2×217px at max 150), unlabeled dark cells | ceil semantics: 3×145, Flutter parity (ph1); boa-design restyle — 500 labeled hue tiles on the (i·47)%360 ramp, white 11px indices, per-cell 2px padding standing in for boa's 4px grid spacing (round-4 ph1) | ✅ PASS — round-4 re-audit: boa's bright labeled-tile design; synthetic-wheel scroll mounts deep windows (0–19 → 245–263), boa matches |
| 11 | lazy-grid-gallery | MAJOR — header collapsed to its child; floor columns | header is a full-width flush-left band (`CROSS_ALIGN_STRETCH`) (ph5); ceil columns (ph1); full boa restyle — readable header + "6000 tiles · only the visible rows mount" subtitle, Square/Wide/Tall + Normal/Dense chips, rounded labeled tiles over boa's own parameters (maxCross 140/85, aspect derive, 6000 tiles) (round-4 ph1) | ✅ PASS — round-4 re-audit: chips reshape/re-column (Dense 3→5), selected fills swap, labels readable, wheel lazy-mounts; the grid re-mounts on a mode switch (scroll resets — sanctioned). The default column-count difference vs the boa viewer (3×~131px vs 2×~200px) traces to pane width + boa's column math, not the case — design matched, boa's rendering of it not ported |
| 12 | lazy-grid-scroll | MAJOR — floor columns; wheel scrolled nothing anywhere | ceil columns (ph1); window-level non-passive wheel listener + deltaMode normalization in the wasm shell (ph3) | ✅ PASS — wheel scrolls + lazy-mounts new rows (zebra style note as #10) |
| 13 | lazy-list-var-sizes | MINOR — extents worked; no axis flip, unreadable rows | axis-flip toggle (Condition label + one-item Each remount), sine-hash extents via `math_sin`, colored width-bars + h/bar labels, zebra; horizontal variant with w= labels (ph8) | ✅ PASS — flip + bars |
| 14 | lazy-list-virtualized | MINOR — bare dark rows | boa 10,000-contact list: colored initials avatars, names, "Item #i of 10000" subtitles, zebra, 56px extent (ph8) | ✅ PASS — 1:1 with boa; wheel advances deep |
| 15 | password-input | MAJOR — typed text invisible (keys never reached va children) | va-child keyboard/focus forwarding — keys+IME focus-routed into the focused child instance (ph2); `obscure_bound` reactive reveal + Show/Hide pill (ph6+ph8); boa design: plain + password + `value: "…"` readout (ph8) | ✅ PASS — live bullets, reveal round-trip, live readout |
| 16 | table-basic | MAJOR — flex-row stub, 4 planets, no stripes, not the Table element | rebuilt on `Table()`: fixed 150 / flex 1 / flex 2 (min 120) columns, 6 planets, PLANET/NOTES/DISTANCE header, intrinsic row extents so notes wrap (ph8); stripes now declarative — `table_stripe` + `table_col_extent` rows replace the cell-painted adaptation (round-4 ph3) | ✅ PASS — round-4 re-audit: stripes pixel-exact (#0b1220 / #1e293b, full-width, row-aligned), wrapped notes; boa clips Saturn's third line (Appendix A #5) |
| 17 | table-reactive | MAJOR — both sides broken differently: rut header scrambled to the top-right; boa's async body never populates | verdict: case-authoring — `CROSS_ALIGN_STRETCH` makes the header a full-width flush-left band; add-row band spans the width, tap fires across it (ph5). Boa's empty body is a reference defect (Appendix A) | ✅ PASS — band + populated body + working add-row (boa's body is still empty) |
| 18 | text-demo | MAJOR — one dead control (the cycle-maxLines button) on a near-perfect port | verdict: tooling artifact — the probe's CDP wheel fired at (0,0); real wheel + taps work; case unchanged and correct (ph4) | ✅ PASS — cycles 2→1→3→2, caption + cards follow |
| 19 | todolist | MAJOR — both sides flawed: rut `it_tap` was an empty stub with invisible checkboxes; boa boots a stuck modal | full boa Tasks port: "Tasks — N items · M done" derive header, New Task modal (fresh controllers at activation), remove-confirm via `then_build` (reads the targeted task at open time), colored checkboxes, `it_tap` finished (toggle rewrites the list atom); boa's boot-modal bug not ported (ph8) | ✅ PASS — toggle/add/remove journeys all live |

**Re-audit verdict: 17 PASS / 2 MINOR / 0 MAJOR** — the accept bar holds (no
MAJOR verdicts outside the documented engine walls). The two MINORs are
authored-styling gaps on the lazy-grid pair (rows 10–11; function is green,
content predates the boa-design restyles the eager grids got in ph7) —
recorded as follow-ups, not repaired here (a content redesign, outside this
phase's docs+audit scope). Smaller notes: table-reactive's row text runs low
contrast on the dark frame; implicit-animations' compact geometry is a plain
pill where boa pads a card.

**Engine walls documented during the plan — limitations, not regressions:**
a switch branch mounting another switch mid-flush defers the inner
activation (grid-gallery stays flat by design; the flattening workaround:
re-key ONE flat switch over the combined state string — revisit when a
real consumer needs same-flush nesting). This is the ONE
documented-remaining wall. The other three closed in round 4:
`el_transform` takes bound angles now (`rotate_bound`/`rotate` —
complex-animation no longer rotates through the rebuild channel), the
Table family gained declarative `table_stripe` / `table_col_extent`
rows, and the embedded font chain covers U+2318 (the status bar's ⌘
renders).

### Round 4 — the reaudit-findings closing round (2026-10-06)

Round 4 landed the round-3 follow-ups: the two MINOR lazy-grid restyles
(565affd), the bound-transform rows (93a47a6), the declarative Table
stripe/extent rows (c77f78f) and U+2318 font coverage (8f59065). The
targeted re-audit (fresh wasm byte-verified == dist at 8f59065, rut 8080
vs the boa reference 8081, same method as the original sweeps) returned
**5/5 PASS**:

- **lazy-grid-basic** ✅ — boa's labeled-tile design: bright hue tiles,
  white indices readable; synthetic-wheel scroll mounts deep windows
  (0–19 → 245–263); boa matches tile-for-tile.
- **lazy-grid-gallery** ✅ — readable flush-left header + "6000 tiles"
  subtitle; Square/Wide/Tall reshape the tiles, Normal/Dense re-columns
  3→5, selected chip fills swap; rounded labeled tiles scroll and
  lazy-mount. The case carries boa's parameters verbatim (maxCross
  140/85, aspect derive, 6000 tiles); the observed default column-count
  difference vs the boa viewer (3×~131px vs 2×~200px) traces to pane
  width + boa's column math — design matched, boa's rendering of it not
  ported. Sanctioned delta: the grid re-mounts on a mode switch (scroll
  resets) where boa reflows in place.
- **complex-animation** ✅ — rotation rides `rotate_bound(progress)`:
  the inner square spins smoothly through looping play with
  `elementTree()` sampled 3× mid-play byte-identical and `frameStats`
  clean (flushes == paintedFrames, dirtyLayoutNodes 0) — zero per-tick
  remount (the round-3 rebuild channel is gone); width/radius/hue/orbit/%
  all animate; badge FORWARD → COMPLETED.
- **table-basic** ✅ — declarative stripes pixel-exact: even rows
  #0b1220 / odd rows #1e293b, full-width and row-aligned, no gaps;
  notes wrap (boa clips Saturn's third line — Appendix A #5).
- **status bar** ✅ — "⌘S to run" renders the real place-of-interest
  sign (HiDPI 3× zoom: four corner loops + center cross), no tofu.

Walls drop to the one documented-remaining (above). Round-4 note: the
auto-run toggle state resets on reload (playground chrome, pre-existing,
not a case concern).

**Audit tooling notes (both sweeps):** `agent-browser mouse wheel` /
`scroll` never deliver wheel events to the canvas (boa's reference is
equally frozen — this poisoned the original audit's "wheel dead everywhere"
verdicts; use a synthetic `WheelEvent` or raw CDP at real coordinates), and
`keyboard type` (CDP `insertText`) produces no `keydown`s, so the engine
never sees it — type with per-char `press`. Real trusted wheel/keys work in
both playgrounds.

### Round 5 — the mutation rail + the case-corpus idiom sweep (2026-10-06)

Round 5 ran the `round5b-mutation-rail` plan in five commits: the rut pin
bump 3165f93 → 442a979 (ef3cf26), the mutation rail (c17ea9b), the corpus
idiom sweep (36c4b04), the remaining round-5 case fixes (545bd46), and
gates + fresh-wasm browser verification (bf95b40) — closing the round-5
operator audit (19 showcase cases × both playgrounds, rut 8080 vs the boa
reference 8081, plus the playground chrome; the audit drove every case's
journey and m4 re-drove the fixed set on fresh, byte-verified wasm).

**The mutation rail (architecture note).** boa's reactive triad is
COMPLETE: `source_*` mints a typed `Source<T>` handle, `derive_*` mints a
`Readable<T>` (sources ∪ derives — one class), and `mutate` seals a
`Mutation` box over a `MutationCtx`. **`on_click(mutate(...))` is the
handler surface** — `on_tap`, the `2` twins and `.ids()`/`.id()` are gone
— and every other callback prop rides the same sealed-mutation surface
(`on_input`, `on_enter`/`on_exit`, `on_key_down`, `on_focus`/`on_blur`,
`on_mount`, the anim `on_tick`/`on_end` twins, `net_stream`'s
`on_chunk`). State flows by capture, event data by typed argument
(`PointerEvent` with nested `Point`s), writes flow through `ctx.set_*`,
reads through `ctx.get_*` (tracked — a derive's `ctx.get` records its
deps), composition through `ctx.run*`. Mutations are invoked at the
flush's MUTATION PASS — never synchronously inside the gesture dispatch.
Async journeys take their handles as parameters over a task-scoped
`TaskCtx` from `spawn` — the stash rails (`st_*`/`stf_*`/`peek`) and the
raw `rs_*` rows never appear in case code. Documented walls (the m1
spike, settled against rut 442a979): ctx reads/writes spell per kind
(`ctx.get_f64`/`ctx.set_f64`, … — the element type is an OUTPUT, so no
generic inference), `mutate` is arity-split (`mutate` nil-arg /
`mutate_ev` PointerEvent / `mutate_f64` typed arg), `spawn` is
call-shaped (`spawn(work(TaskCtx.mint(), handles…))`), and `ctx.run*`
QUEUES the composed invocation (drained at the next mutation pass —
effects read through sources; there is no synchronous return).

#### The 19-case journey ledger (both audit parts)

| # | case | round-5 audit verdict | what changed (round 5) | journey status |
|---|------|-----------------------|------------------------|----------------|
| 1 | complex-animation | MINOR — all 5 flagged deltas confirmed + the curve-switch addendum (boa seeks the DISPLAYED value, tur re-eased from raw) | rewritten to boa parity (opaque inner square, center-orbit at boa's (140,80), "Loop ✓" in white, 4/32/32 spacers, rounded % readout, curve-switch seeks the DISPLAYED value); transport/speed/curve/loop are mutations, the % readout a derive (m2) | ✅ PASS — Play/Pause (pixel-frozen stage)/Resume/Reverse/Stop, speed-chip highlight, looping wrap past 100% without COMPLETED, completed card exactly 280px |
| 2 | implicit-animations | MINOR — box animated size (boa's is fixed), no shadow anywhere, subtitle text | rewritten: FIXED 150×160 card (only radius/color/shadow animate), the shadow present (bound blur 16↔32), boa's "AnimatedContainer · AnimatedOpacity · AnimatedPositioned" subtitle; the toggle is one mutation (m2) | ✅ PASS — both ends + slide 30↔160, Compact 0.45 whole-card opacity parity |
| 3 | jigsaw-puzzle | MAJOR — no shadows/glow, no lift, an invented cyan snap-highlight, Solved! banner top-left without a scrim | rewritten: drag shadow + placed pieces glowing their own hue, the 180ms lift controller (held at LIFT_MAX while dragged — the `a_lift_end` fix), Solved! full-viewer scrim with the banner viewer-centered (the Positioned verbatim-edge fix); drag = `on_pointer_down/move/up` mutations, captures replace `.ids()` (m2) | ✅ PASS — full solve incl. glow/lift/Solved! scrim; post-solve Shuffle absorbed |
| 4 | countdown | MINOR — pill states collapsed 4→2, urgent-red missing, 96px display | m3: the four-state pill (Ready/Running/Paused/Done), the urgent-red display ≤10s while running (a Condition over the urgent source), 72px display | ✅ PASS — full flow incl. Done + Restart |
| 5 | counter | PASS | swept onto `on_click(mutate(...))` (m2) | ✅ PASS — 12 − 2 = 10, every click registered |
| 6 | todolist | MAJOR on the BOA side — the stuck boot-modal (Appendix A #1); tur effectively PASS | boa's boot-modal bug deliberately not ported; swept onto captured controllers + source arrays (m2) | ✅ PASS — check → 2 done, remove-confirm quoting the task, add appends ("4 items · 2 done") |
| 7 | table-reactive | MAJOR — tur implemented a DIFFERENT case (an add-row probe); boa's body also broken | full boa port (m3): async 300ms fake load, sortable PLANET/MOONS/GRAVITY headers both directions with active-fill + ^/v markers, a Loading… → "Loaded 8 rows" status derive, WORKING rows (boa's empty body = Appendix A #2) | ✅ PASS — 8 rows render, MOONS/GRAVITY sorts both directions |
| 8 | table-basic | PASS | no interactive surface; boa's Saturn notes clip = Appendix A #5 (the round-4 verdict stands) | static — audit boot renders, no regression |
| 9 | password-input | MINOR — a tur-only Show/Hide pill; column alignment | m3: the invented pill dropped — boa masks permanently, the value echo is the truth channel (static `.obscure(true)`, the readout a direct column child); the start-aligned column stands as a recorded styling note | ✅ PASS — static mask, caret, live readout ("hunter2x") |
| 10 | github-viewer | MINOR — crumb not clickable, the crumb read "react/react", full-pane loading, folder-nav untestable (GitHub rate limit) | finished (m2): crumb tap, list-area loading, the crumb = the parsed draft identity ("facebook/react") | ✅ PASS live (quota reset) — browse → explorer → descend into packages → crumb tap back to root → Back to the re-prefilled landing |
| 11 | grid-aspect | MINOR — column counts (boa under-divides); tur's deliberate second grid + captions + ScrollView | design divergence recorded — tur's ceil is the Flutter parity, boa's under-divide = Appendix A #6; swept only | static — audit boot renders, no regression |
| 12 | grid-basic | MINOR — column counts; boa's 4th row clips with no scroll | recorded (the same grid-math split, Appendix A #6); swept only | static — audit boot renders, no regression |
| 13 | grid-gallery | MINOR — tile labels centered vs top-left; column counts | m3: labels top-left (boa's alignment-less Container child). The journeys FOUND + FIXED a real bug (m4): the VM binds a mutation box's capture cell per CALL SITE, so the loop's one tile call shared one cell across every pad (every tap answered the last index) — unrolled literal tile calls + the nine mode grids pre-built eagerly, pinned red→green by the tile-tap journey | ✅ PASS — chips reshape/re-column, the ring follows the tapped tile |
| 14 | lazy-grid-basic | PASS | swept only | ✅ the viewer wheel-sanity case — 6000px + 15000px deep + return smooth, correct windowing, both-end clamping |
| 15 | lazy-grid-gallery | MINOR — "6000" vs boa's "6,000"; column counts; boa-side label uncertainty at depth | m3: the subtitle now "6,000 tiles" (the toLocaleString twin); column counts stay the recorded grid-math split | ✅ PASS — "6,000 tiles" verified |
| 16 | lazy-grid-scroll | MAJOR — tur implemented a DIFFERENT case (600 zebra cells) | full boa port (m3): 5000 cells over the (i·37)%360 hue ramp @ 50/45, fixed 60px rows, 6px gaps — new `lazy_grid_item_extent`/`lazy_grid_spacing` rows + `LazyGrid.item_extent()`/`.spacing()` kit methods (TDD red-first pitch test) | ✅ PASS — hue ramp, deep windowing, pixel-identical top return |
| 17 | lazy-list-var-sizes | MINOR — the 12-slot palette twin vs boa's continuous hsl | sanctioned deliberate twin (per source, recorded); swept only | audit journeys (deep + horizontal) — the sanctioned twin stands |
| 18 | lazy-list-virtualized | PASS — one shared NOTE: huge wheel deltas desync names from subtitles on BOTH engines (Appendix A #7); the palette twin | tur replicates the reference bit-for-bit; swept only | audit journeys (deep + huge-jump + top return) — bit-for-bit with boa |
| 19 | text-demo | PASS | swept only | audit journeys (overflow + maxLines cycles) — section-for-section pixel-equivalent |

Playground chrome (the audit's third block): **A** — the sidebar
case-list wheel is parity (both scroll; 1:1 px mapping; both-end
clamping; no viewer misrouting). **B** — the REAL tur gap the audit
measured (the editor pane frozen: no wheel translation, no caret-follow)
is CLOSED — the multiline-Input scrolling engine work rode the sweep (m2,
+7 tests) and the m4 browser journey verified wheel translates the view
and ArrowDown caret-follow scrolls past the viewport bottom. **C** —
viewer-pane wheel sanity PASS (lazy-grid-basic, #14 above).

**Follow-ups** (what the phase ledgers say remains open):

- Per-keystroke editor highlighting stays the recorded phase-D decision
  (highlight at case load + after a successful Run, never per keystroke —
  typed text inherits the caret span's ink); the rut-semantic classifier
  crate (new at the 442a979 pin, not taken this round) is the future
  optional dep that could revisit it.
- The highlight palette carries the seven `code.*` rows the lexical
  tokenizer can distinguish — boa's property/parameter palette rows are
  deferred.
- The VM's per-CALL-SITE mutation capture cell (a loop-built pad shares
  one cell across every tap) is documented at the workaround site
  (grid-gallery's unrolled literal tiles — jigsaw's `piece()` law);
  revisit when the VM binds capture cells per closure.
- The ctx surface spells per kind (`ctx.get_f64`/`ctx.set_f64`, …;
  `mutate`/`mutate_ev`/`mutate_f64`) and `ctx.run*` composes by queued
  invocation — the settled, boa-legible shape. A generic `ctx.get<T>` /
  single-name `mutate` returns only if the VM ever grows return-position
  generic inference, fn-value asyncs, and re-entrancy.

### Appendix A — boa reference defects (do not port; "match boa's designs, not boa's bugs")

1. **todolist** boots with a stuck "Remove task?" modal + scrim on every
   fresh load; Cancel doesn't dismiss it, locking the viewer (re-confirmed in
   the re-audit; round 5: the modal is UNDISMISSABLE — Cancel, backdrop,
   Remove, and Escape all dead across two independent boots — with an EMPTY
   body, no task name).
2. **table-reactive**'s async body never populates — the table stays empty
   (re-confirmed in round 5: headers sort but no rows and no loading
   indicator ever render, even after sort clicks; rut's ported rows
   populate and sort — m3).
3. **jigsaw-puzzle**'s placed-counter stays "0 / 9" after correct drops.
4. **github-viewer**'s error banner renders empty (a pink strip, no text) —
   compare rut's full-message banner.
5. **table-basic**'s Saturn row clips its third notes line ("its ring
   system" cut off after "its ring") — rut's intrinsic row extents show
   all three lines (re-confirmed in round 4).
6. **Grid column math under-divides** — boa floors its column count,
   violating its own `maxCrossAxisExtent` (grid-basic 3 × ~141px at max
   140; grid-aspect 2 × ~223px at max 160; grid-gallery Normal 186 > 150
   and Sparse a single 381px-wide column at max 220). Boa is
   self-consistent with its own floor comment, but its overflowing last
   rows clip at the pane bottom with no scroll path; tur's ceil is the
   Flutter parity — the recorded design divergence (grid column-math
   untouched, m3).
7. **Huge-jump wheel desync (shared)** — one huge single-event wheel
   delta (~30,000px) desyncs lazy-list-virtualized's row NAMES from its
   sequential, index-true subtitles on BOTH engines identically (name
   runs stay internally consistent but sit at a varying offset;
   normal-magnitude wheels ≤ 600px/event keep identity perfect on both).
   Shared engine behavior — recorded, not a tur delta.
