# tur-test-cases — the shared case corpus (rut)

The case corpus drives three consumers:

1. **Rust integration tests** — `TurTestApp::load_bundle` / `load_rut_bundle`
   load a case by name (`cases/<name>/index.rut` for the rut rail; the
   legacy `dist/<name>.js` path is the JS rail's, retired with Phase 4).
2. **The playground** — `demo/playground-view/scripts/gen-cases.cjs` embeds
   the case sources verbatim so the sidebar can list + open every case.
3. **Rut-semantics documentation** — the cases are the reference examples
   for authoring tur UIs in rut (see the `tur_kit` prelude —
   `libs/tur-engine/src/kit/tur_kit.rut` — for the builder classes each
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
- **no JS anywhere** — a rut case never touches a JS realm. State lives in
  atoms (`rs_source_str` / `rs_source_f64` / `rs_source_bool` / value
  atoms) whose ids cross into entries as plain `u64`s. Tests read state
  back through **dev-tool tree queries** (`query_element`,
  `dev_tool_element_tree`, `query_text`) or bound atoms — never by
  evaluating script.

### Conventions

- **Naming**: the directory name is the case name (`kebab-case`), e.g.
  `cases/counter/index.rut` loads as `"counter"`.
- **Query keys**: give every element a test needs to find a query key via
  the element's chainable `.query_key("key")` method; tests locate it with
  `app.query_element(&["key"])`.
- **Callbacks are fn values** — plain `fn`s (conventionally prefixed by
  their role: `ts_` test-seam actions, `g_` gesture, `f_` focus, `a_`
  animation, `on_` watch / chunk deliveries), passed to the kit by name
  or as anonymous fn literals (`PointerInteract().on_tap(b_toggle)`,
  `Each(items).item_builder(fn(i: u64, item: str) -> View { … })`). The
  kit checks the arity/types at compile time; the pump fires the callback
  through the infra dispatch entries with the same payload shapes as
  always.
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
first — the follow-up queue:

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
use tur::{ mount, rs_get_f64, rs_set_f64, rs_set_str, rs_source_f64, rs_source_str };
use tur_kit::{ Column, Container, PointerInteract, Text };

entry fn start() -> u64 {
    let count = rs_source_f64();
    let label = rs_source_str("Count: 0");
    let col = Column()
        .query_key("col")
        .child(Text().text_bound(label).query_key("count").build())
        .child(button(count, label, ts_inc, "+1"));
    mount(col.build());
    return count;
}

// A pill button: a PointerInteract pad (the tap delivers `(a, b, seq)`)
// wrapping a styled label — the el_button composite, authored from families.
// The callback is a FN VALUE (compile-time arity/type checked at the kit).
fn button(count: u64, label: u64, cb: fn(u64, u64, f64), text: str) -> opaque {
    return PointerInteract()
        .ids(count, label)
        .on_tap(cb)
        .child(
            Container()
                .color(0x6366F1FFu64)
                .child(Text().text(text).build())
                .build(),
        )
        .build();
}

fn ts_inc(count: u64, label: u64, _n: f64) {
    rs_set_f64(count, rs_get_f64(count) + 1.0);
    rs_set_str(label, f"Count: {rs_get_f64(count) as u64}");
}
```

The test side:

```rust
let mut app = TurTestApp::new(400.0, 600.0).unwrap();
app.load_rut_bundle("counter").unwrap();
assert_eq!(app.query_text(&["count"]).as_deref(), Some("Count: 0"));
let atom = app.rut_start_answer();
app.call_rut_entry("ts_inc", atom, 0.0).unwrap();
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
| 1 | complex-animation | MINOR — no orbit dot (no trig rows), no rotation channel, control rows hugged left | `math_sin`/`math_cos` + `radius_bound` rows (ph6); rows centered, rotation restored via the rebuild channel (a one-item Each re-mounts the square under a fresh Transform — `el_transform` takes static angles only), orbiting dot on bound Positioned anchors, radius animates 8→40 (ph8) | ✅ PASS — dot orbits, square rotates, width/hue/% animate |
| 2 | countdown | MAJOR — ~16px display, four flat pills, top-aligned, no status chip | full boa styling: 96px display + COUNTDOWN eyebrow, centered pane, ● Ready/Running pill, big Start⇄Pause Condition swap; edit modal via `then_build` (fresh controller prefilled with the live `initial` — the re-seed workaround dropped); mid-tick-edit clock-freeze fix + regression test (ph7) | ✅ PASS — full flow incl. edit modal + reset |
| 3 | counter | MINOR — button row hugged the left edge (expanding `gap()`) | gap dropped, `MAIN_ALIGN_CENTER` + page centering; count readout rides a derive (ph8) | ✅ PASS — row centered |
| 4 | github-viewer | MINOR — "GH" text badge, no Repository label, no prefill, off chip set | octocat SVG badge (`img_res_svg`), "Repository" label, prefill "facebook/react", boa chip set facebook/react · tailwindlabs/tailwindcss · vuejs/core (ph8) | ✅ PASS — Browse renders the live explorer |
| 5 | grid-aspect | MAJOR — three solid tiles, no Grid | grid ceil semantics (ph1); full port: `Grid().max_cross(160).aspect(2.0)` of 10 labeled hue cells + a second grid exercising `.main_extent(40)` (ph7) | ✅ PASS — both grids match |
| 6 | grid-basic | MAJOR — 2×2 static slab | ceil semantics (ph1); 12 hue-ramp cells on `Grid().max_cross(140).spacing(8,8)` (ph7) | ✅ PASS — 12 labeled cells; rut's 4 columns vs boa's 3 is the sanctioned Flutter ceil (boa floors) |
| 7 | grid-gallery | MAJOR — three dark tiles; no header/chips/derives | ceil semantics (ph1); header band via `CROSS_ALIGN_STRETCH` (ph5); full port: live derive subtitle ("tile #i · 1:1 · maxExtent 150"), aspect + density chips, scrollable 27-tile grid, tap-to-select rings; chips re-key one flat Switch over the combined mode string (deliberately avoiding the nested-switch wall) (ph7) | ✅ PASS — 16:9 reshapes + subtitle; Dense re-keys 3→5 columns; ring follows taps |
| 8 | implicit-animations | MAJOR — stub: empty `a_tick`, static box, tap did nothing | wired end-to-end: 600ms easeInOut controller drives width 100→200 / height 40→80 / color sky→indigo / radius 12→40 (`radius_bound`) / slide / label fade; Compact↔Expanded via Condition (ph8) | ✅ PASS — mid-flight + settled shots |
| 9 | jigsaw-puzzle | MAJOR — one dead fixture piece (double-build authoring; drag dead) | single-build authoring + bound label; hit-test verdict: case-authoring, engine sound (ph4); full 3×3 game: 9 pieces on bound anchors, snap+lock, "N / 9 placed" derive badge, Shuffle (Park–Miller LCG), Solved! banner; native pins drive the full loop (ph9) | ✅ PASS — snap highlight, lock, 1/9; wrong drop refused; Shuffle re-deals |
| 10 | lazy-grid-basic | MAJOR — floor columns (2×217px at max 150), unlabeled dark cells | ceil semantics: 3×145, Flutter parity (ph1) | ⚠️ MINOR — functional (3-col virtualized grid, wheel-scrolls) but the authored tiles are still the dark zebra style vs boa's bright labeled hue cells — follow-up: restyle to the boa design |
| 11 | lazy-grid-gallery | MAJOR — header collapsed to its child; floor columns | header is a full-width flush-left band (`CROSS_ALIGN_STRETCH`) (ph5); ceil columns (ph1) | ⚠️ MINOR — band + labeled 3-col grid + wheel lazy-mounts; the header label is dark-on-dark and boa's chip row is absent — follow-up: restyle to the boa design |
| 12 | lazy-grid-scroll | MAJOR — floor columns; wheel scrolled nothing anywhere | ceil columns (ph1); window-level non-passive wheel listener + deltaMode normalization in the wasm shell (ph3) | ✅ PASS — wheel scrolls + lazy-mounts new rows (zebra style note as #10) |
| 13 | lazy-list-var-sizes | MINOR — extents worked; no axis flip, unreadable rows | axis-flip toggle (Condition label + one-item Each remount), sine-hash extents via `math_sin`, colored width-bars + h/bar labels, zebra; horizontal variant with w= labels (ph8) | ✅ PASS — flip + bars |
| 14 | lazy-list-virtualized | MINOR — bare dark rows | boa 10,000-contact list: colored initials avatars, names, "Item #i of 10000" subtitles, zebra, 56px extent (ph8) | ✅ PASS — 1:1 with boa; wheel advances deep |
| 15 | password-input | MAJOR — typed text invisible (keys never reached va children) | va-child keyboard/focus forwarding — keys+IME focus-routed into the focused child instance (ph2); `obscure_bound` reactive reveal + Show/Hide pill (ph6+ph8); boa design: plain + password + `value: "…"` readout (ph8) | ✅ PASS — live bullets, reveal round-trip, live readout |
| 16 | table-basic | MAJOR — flex-row stub, 4 planets, no stripes, not the Table element | rebuilt on `Table()`: fixed 150 / flex 1 / flex 2 (min 120) columns, 6 planets, PLANET/NOTES/DISTANCE header, stripes cell-painted (the kit Table exposes no stripe row — sanctioned adaptation), intrinsic row extents so notes wrap (boa's `rowExtent(36)` would clip — design over bug) (ph8) | ✅ PASS — 6 planets, stripes, wrapped notes |
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
`el_transform` takes static angles only (complex-animation rotates through
the rebuild channel); the kit Table exposes no stripe/extent rows (stripes
are cell-painted case-side); a switch branch mounting another switch
mid-flush defers the inner activation (grid-gallery stays flat by design);
the embedded fonts lack U+2318 (the status bar's "⌘S to run" hint shows a
tofu box when shown — playground chrome, not a case).

**Audit tooling notes (both sweeps):** `agent-browser mouse wheel` /
`scroll` never deliver wheel events to the canvas (boa's reference is
equally frozen — this poisoned the original audit's "wheel dead everywhere"
verdicts; use a synthetic `WheelEvent` or raw CDP at real coordinates), and
`keyboard type` (CDP `insertText`) produces no `keydown`s, so the engine
never sees it — type with per-char `press`. Real trusted wheel/keys work in
both playgrounds.

### Appendix A — boa reference defects (do not port; "match boa's designs, not boa's bugs")

1. **todolist** boots with a stuck "Remove task?" modal + scrim on every
   fresh load; Cancel doesn't dismiss it, locking the viewer (re-confirmed in
   the re-audit).
2. **table-reactive**'s async body never populates — the table stays empty
   (re-confirmed; rut's body populates and adds rows).
3. **jigsaw-puzzle**'s placed-counter stays "0 / 9" after correct drops.
4. **github-viewer**'s error banner renders empty (a pink strip, no text) —
   compare rut's full-message banner.
