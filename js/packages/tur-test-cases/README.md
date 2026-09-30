# tur-test-cases — the shared case corpus (rut)

The case corpus drives three consumers:

1. **Rust integration tests** — `TurTestApp::load_bundle` / `load_rut_bundle`
   load a case by name (`cases/<name>/index.rut` for the rut rail; the
   legacy `dist/<name>.js` path is the JS rail's, retired with Phase 4).
2. **The playground** — `demo/playground-view/scripts/gen-cases.cjs` embeds
   the case sources verbatim so the sidebar can list + open every case.
3. **Rut-semantics documentation** — the cases are the reference examples
   for authoring tur UIs in rut (see `libs/tur-engine/src/core/rut_runtime/`
   for the row surface each example uses).

## The per-case contract (rut)

Every case is a **single `index.rut` module** with:

- **`entry fn start()`** — the ONLY mount point. It authors the tree
  through the `tur` host rows (`el_column`, `el_text`, `el_box`, …) and
  hands the root to the engine with `mount(el_build(root))`. `start` may
  return `-> u64` (the host records the answer — conventionally the id of
  the module's root state atom — readable via `TurTestApp::rut_start_answer`).
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
  the builder's `el_qkey(builder, "key")` row; tests locate it with
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

### Example

```rut
use tur::{ el_button, el_column, el_qkey, el_text_bound, el_build, el_child,
           mount, rs_get_f64, rs_set_f64, rs_set_str, rs_source_f64, rs_source_str };

entry fn start() -> u64 {
    let count = rs_source_f64();
    let label = rs_source_str("Count: 0");
    let col = el_column();
    el_child(col, el_qkey(el_text_bound_new(label), "count"));
    el_child(col, el_button(count, label, "ts_inc", "+1"));
    mount(el_build(col));
    return count;
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
