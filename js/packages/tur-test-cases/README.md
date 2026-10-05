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
  `app.call_rut_entry(name, a, b)` (the engine→rut event rail: the same
  shape element callbacks receive). Element callbacks
  (`el_button`/`el_gesture`/`el_focusable`/`anim_ctrl` onTick/watch
  deliveries) name their `entry fn` and the pump drains the intents into
  them. Probes replace the JS era's `eval_js` state pokes.
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
- **Callbacks**: `entry fn` names are conventionally prefixed by their
  role (`ts_` for test-seam actions, `g_` gesture, `f_` focus, `a_`
  animation, `on_` watch / chunk deliveries) but any name works — the
  engine resolves the callback's string at intent-drain time.
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
        .child(button(count, label, "ts_inc", "+1"));
    mount(col.build());
    return count;
}

// A pill button: a PointerInteract pad (the tap delivers `(a, b, seq)`)
// wrapping a styled label — the el_button composite, authored from families.
fn button(count: u64, label: u64, cb: str, text: str) -> opaque {
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

entry fn ts_inc(count: u64, label: u64, _n: f64) {
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
