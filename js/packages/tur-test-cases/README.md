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
