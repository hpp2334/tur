//! Phase M1 — the mutation rail: `source` / `derive` / `mutate` (boa's
//! reactive triad) over the `MutationCtx` / `DeriveCtx` / `TaskCtx` ctxs.
//!
//! The plan's (a)–(g) list, pinned end-to-end through real gesture events:
//! the click mutation's write lands through the flush's mutation pass
//! (queued — never synchronously inside the gesture dispatch), captures
//! replace the stash, `ctx.run` composes, the ctx reads reach sources AND
//! derives, drag mutations receive one typed `PointerEvent`, spawned tasks
//! carry a task-scoped ctx across awaits, and nothing remounts.

use std::time::Duration;

use tur_integration_tests::TurTestApp;

fn rut_bound_text(app: &TurTestApp) -> String {
    let id = app
        .query_element(&["rut", "text"])
        .expect("bound text not found");
    let id = tur_engine::core::element::ElementNodeId::new(id.as_u64());
    app.with_element(id, |e| {
        e.cast::<tur_engine::builtin_plugins::text::TextElement>()
            .map(|c| c.spans().iter().map(|s| s.text.as_str()).collect::<String>())
            .unwrap_or_default()
    })
    .unwrap_or_default()
}

fn rut_text_at(app: &TurTestApp, key: &'static str) -> String {
    let id = app.query_element(&["rut", key]).expect("text not found");
    let id = tur_engine::core::element::ElementNodeId::new(id.as_u64());
    app.with_element(id, |e| {
        e.cast::<tur_engine::builtin_plugins::text::TextElement>()
            .map(|c| c.spans().iter().map(|s| s.text.as_str()).collect::<String>())
            .unwrap_or_default()
    })
    .unwrap_or_default()
}

/// An element's center in canvas coordinates, found through its query key.
fn element_center(app: &TurTestApp, key: &'static str) -> (f64, f64) {
    let id = app
        .query_element(&["rut", key])
        .unwrap_or_else(|| panic!("element {key} not found"));
    let n = app
        .dev_tool_get_element(tur_engine::core::element::NodeId::from(
            tur_engine::core::element::ElementNodeId::new(id.as_u64()),
        ))
        .unwrap();
    (n.absolute.0 + n.size.0 / 2.0, n.absolute.1 + n.size.1 / 2.0)
}

// ---------------------------------------------------------------------------
// (a) + (c) — the triad: a click mutation's `ctx.set` lands and the bound
// label repaints. The mutation CAPTURES the sources minted in `start()` —
// no stash, no ids. The label is a first-class derive (the boa counter).
// ---------------------------------------------------------------------------

const COUNTER_RUT: &str = r#"

use tur_kit::{ Column, DeriveCtx, Mutation, MutationCtx, PointerInteract, Readable, Source, TaskCtx, Text, derive, mount, mutate, source };

entry fn start() -> u64 {
    let n: Readable<f64> = source<f64>(0.0);
    let label: Readable<str> = derive<str>(fn (ctx: DeriveCtx) -> str {
        return f"{ctx.get<f64>(n) as u64} clicks";
    });
    let bump = mutate(fn (ctx: MutationCtx, _e: nil) {
        ctx.set<f64>(n, ctx.get<f64>(n) + 1.0);
    });
    let col = Column()
        .child(Text().text_bound(label).query_key("rut/text").build())
        .child(PointerInteract().on_click(bump).query_key("rut/pad").child(Text().text("tap").build()).build());
    mount(col.build());
    return n.atom_id();
}
"#;

#[test]
fn a_click_mutation_writes_and_the_derived_label_repaints() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_rut_module(COUNTER_RUT).unwrap();
    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(rut_bound_text(&app), "0 clicks", "the derive rendered initially");

    let (px, py) = element_center(&app, "pad");
    for n in 1..=3u64 {
        app.click(px, py);
        app.wait_for_timeout(Duration::ZERO);
        assert_eq!(
            rut_bound_text(&app),
            format!("{n} clicks"),
            "click {n} landed through ctx.set"
        );
    }
}

// ---------------------------------------------------------------------------
// (b) — queued semantics: the write does NOT happen synchronously inside
// the gesture dispatch. One pointer batch (down → up → the synthesized
// click) enqueues THREE invocations; they drain in enqueue order, and the
// store-write observer (`rs_watch` on the written atom) fires only after
// the write flowed through the reactive flush.
// ---------------------------------------------------------------------------

const QUEUED_RUT: &str = r#"
use tur_host::{ rs_get_str, rs_set_str, rs_watch, rs_watch_start };
use tur_kit::{ Column, DeriveCtx, Mutation, MutationCtx, PointerEvent, PointerInteract, Readable, Source, TaskCtx, Text, derive, mount, mutate, source };

entry fn start() -> u64 {
    let log: Readable<str> = source<str>("");
    let n: Readable<f64> = source<f64>(0.0);

    let m_down = mutate<PointerEvent>(fn (ctx: MutationCtx, _ev: PointerEvent) {
        let t = ctx.get<str>(log);
        ctx.set<str>(log, f"{t}|down");
    });
    let m_up = mutate<PointerEvent>(fn (ctx: MutationCtx, _ev: PointerEvent) {
        let t = ctx.get<str>(log);
        ctx.set<str>(log, f"{t}|up");
    });
    let m_click = mutate(fn (ctx: MutationCtx, _e: nil) {
        let t = ctx.get<str>(log);
        ctx.set<str>(log, f"{t}|click");
        ctx.set<f64>(n, ctx.get<f64>(n) + 1.0);
    });

    // The store-write observer: every write to `n` delivers here, after
    // the write, through the reactive flush (the queued-semantics witness).
    let w = rs_watch(n.atom_id(), fn (a: u64, _b: u64, _v: f64) {
        rs_set_str(log.atom_id(), f"{rs_get_str(log.atom_id())}|w{a}");
    }, n.atom_id());
    rs_watch_start(w);
    // The mint's seed write precedes the activation — the first delivery
    // is that boot differential; clear it so the batch assertions read
    // from a clean base.
    rs_set_str(log.atom_id(), "");

    let pad = PointerInteract()
        .on_pointer_down(m_down)
        .on_pointer_up(m_up)
        .on_click(m_click)
        .query_key("rut/pad")
        .child(Text().text("pad").build())
        .build();
    mount(Column().child(Text().text_bound(log).query_key("rut/text").build()).child(pad).build());
    return log.atom_id();
}
"#;

#[test]
fn b_mutations_drain_queued_in_order_and_the_watch_observes_the_write() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_rut_module(QUEUED_RUT).unwrap();
    app.wait_for_timeout(Duration::ZERO);
    // The mint's seed write precedes the watcher activation — one boot
    // delivery may land; the batch assertions read the tail.
    let boot = rut_bound_text(&app);

    let (px, py) = element_center(&app, "pad");
    app.pointer_down(px, py);
    app.pointer_up(px, py);
    app.wait_for_timeout(Duration::ZERO);

    let transcript = rut_bound_text(&app);
    let base = boot.len();
    let down = transcript[base..].find("|down").expect("down landed") + base;
    let up = transcript[base..].find("|up").expect("up landed") + base;
    let click = transcript[base..].find("|click").expect("click landed") + base;
    assert!(down < up && up < click, "enqueue order preserved: {transcript}");
    assert!(
        transcript[click..].contains("|w"),
        "the store-write observer fired after the click's write: {transcript}"
    );
}

// ---------------------------------------------------------------------------
// (d) — `ctx.run` composes: a mutation invoking another with an arg, the
// return value flowing back; plus the nil-arg composition (the plan's
// crumb shape).
// ---------------------------------------------------------------------------

const RUN_RUT: &str = r#"

use tur_kit::{ Column, DeriveCtx, Mutation, MutationCtx, PointerInteract, Readable, Source, TaskCtx, Text, derive, mount, mutate, source };

entry fn start() -> u64 {
    let out: Readable<f64> = source<f64>(0.0);
    let res: Readable<str> = source<str>("");
    let log: Readable<str> = source<str>("");

    // The typed-arg target: the arg crosses the queue; its "return"
    // lands through the source write (composition reads effects through
    // sources — there is no synchronous return across the VM boundary).
    let add_one = mutate<f64>(fn (ctx: MutationCtx, x: f64) {
        let r = x + 1.0;
        ctx.set<f64>(out, r);
        ctx.set<str>(res, f"r={r as u64}");
    });

    // The nil-arg target (the plan's b_root shape).
    let mark = mutate(fn (ctx: MutationCtx, _e: nil) {
        let t = ctx.get<str>(log);
        ctx.set<str>(log, f"{t}|marked");
    });

    let compose = mutate(fn (ctx: MutationCtx, _e: nil) {
        // The composed invocations QUEUE (rut's no-reentrancy law: a
        // mutation body runs inside a face call — the VM is mid-call — so
        // the nested invocation drains at the next mutation pass). The
        // arg crosses; the target's effect pins.
        ctx.run<f64>(add_one, 21.0);
        ctx.run<nil>(mark, nil);
    });



    let pad = PointerInteract()
        .on_click(compose)
        .query_key("rut/pad")
        .child(Text().text("run").build())
        .build();
    mount(
        Column()
            .child(Text().text_bound(res).query_key("rut/text").build())
            .child(Text().text_bound(log).query_key("rut/log").build())
            .child(pad)
            .build(),
    );
    return out.atom_id();
}
"#;

#[test]
fn d_ctx_run_composes_and_return_values_flow() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_rut_module(RUN_RUT).unwrap();
    app.wait_for_timeout(Duration::ZERO);

    let (px, py) = element_center(&app, "pad");
    app.click(px, py);
    // The composed invocations QUEUE (the no-reentrancy law) — they drain
    // on the following mutation passes; poll until they settle.
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    let settled = loop {
        if rut_bound_text(&app) == "r=22" && rut_text_at(&app, "log") == "|marked" {
            break true;
        }
        if std::time::Instant::now() > deadline {
            break false;
        }
        app.wait_for_timeout(Duration::from_millis(25));
    };
    assert!(settled, "the composition settled: out={:?} log={:?}", rut_bound_text(&app), rut_text_at(&app, "log"));
    assert_eq!(rut_bound_text(&app), "r=22", "the arg crossed and the target's effect landed");
    assert_eq!(rut_text_at(&app, "log"), "|marked", "the nil-arg composition ran");
}

// ---------------------------------------------------------------------------
// (e) — the ctx reads reach sources AND derives inside a mutation: the
// toggle's negated read over a bool source, and a str read through a
// first-class derive.
// ---------------------------------------------------------------------------

const TOGGLE_RUT: &str = r#"

use tur_kit::{ Column, DeriveCtx, Mutation, MutationCtx, PointerInteract, Readable, Source, TaskCtx, Text, derive, mount, mutate, source };

entry fn start() -> u64 {
    let expanded: Readable<bool> = source<bool>(true);
    let n: Readable<f64> = source<f64>(3.0);
    let readback: Readable<str> = source<str>("");
    let mirror: Readable<str> = source<str>("");

    // The derive: a first-class readable over the source.
    let badge: Readable<str> = derive<str>(fn (ctx: DeriveCtx) -> str {
        return f"n={ctx.get<f64>(n) as u64}";
    });

    // The toggle: the negated read (boa's ctx.set(expanded$, !ctx.get(expanded$)));
    // the str mirror rides along (text_bound renders str atoms).
    let b_toggle = mutate(fn (ctx: MutationCtx, _e: nil) {
        let v = ctx.get<bool>(expanded);
        ctx.set<bool>(expanded, !v);
        let nv = !v;
        ctx.set<str>(mirror, f"expanded={nv}");
    });

    // A mutation reading a DERIVE through the same ctx.
    let snapshot = mutate(fn (ctx: MutationCtx, _e: nil) {
        ctx.set<str>(readback, ctx.get<str>(badge));
    });

    let snap = PointerInteract()
        .on_click(snapshot)
        .query_key("rut/snap")
        .child(Text().text("snap").build())
        .build();

    let pad = PointerInteract()
        .on_click(b_toggle)
        .query_key("rut/pad")
        .child(Text().text("toggle").build())
        .build();
    mount(
        Column()
            .child(Text().text_bound(mirror).query_key("rut/text").build())
            .child(Text().text_bound(badge).query_key("rut/badge").build())
            .child(Text().text_bound(readback).query_key("rut/readback").build())
            .child(pad)
            .child(snap)
            .build(),
    );
    return expanded.atom_id();
}
"#;

#[test]
fn e_ctx_reads_reach_sources_and_derives() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_rut_module(TOGGLE_RUT).unwrap();
    app.wait_for_timeout(Duration::ZERO);

    let (px, py) = element_center(&app, "pad");
    app.click(px, py);
    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(rut_bound_text(&app), "expanded=false", "the negated read landed");

    let (sx, sy) = element_center(&app, "snap");
    app.click(sx, sy);
    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(rut_text_at(&app, "readback"), "n=3", "the derive read inside the mutation matched");
}

// ---------------------------------------------------------------------------
// (f) — drag mutations receive `(ctx, ev: PointerEvent)`; the field reads
// pin the drag anchors (`ev.global.x` / `ev.global.y`) and the locals
// differ from the globals by the pad's origin.
// ---------------------------------------------------------------------------

const DRAG_RUT: &str = r#"

use tur_kit::{ Column, DeriveCtx, MouseButton, Mutation, MutationCtx, PointerEvent, PointerInteract, Readable, Source, TaskCtx, Text, derive, mount, mutate, source };

entry fn start() -> u64 {
    let log: Readable<str> = source<str>("");

    let m_down = mutate<PointerEvent>(fn (ctx: MutationCtx, ev: PointerEvent) {
        let t = ctx.get<str>(log);
        let gx = ev.global.x;
        let gy = ev.global.y;
        ctx.set<str>(log, f"{t}|d{gx as u64},{gy as u64}");
    });
    let m_move = mutate<PointerEvent>(fn (ctx: MutationCtx, ev: PointerEvent) {
        let t = ctx.get<str>(log);
        let gx = ev.global.x;
        let gy = ev.global.y;
        let lx = ev.local.x;
        let ly = ev.local.y;
        ctx.set<str>(log, f"{t}|m{(gx - lx) as u64},{(gy - ly) as u64}");
    });
    let m_up = mutate<PointerEvent>(fn (ctx: MutationCtx, ev: PointerEvent) {
        let t = ctx.get<str>(log);
        let mut b = "l";
        if (ev.button == MouseButton.Right) {
            b = "r";
        }
        ctx.set<str>(log, f"{t}|u{b}");
    });
    // (The context menu's right-button decode is pinned by the rut_boot
    // gesture fixture's menu pad — on one pad the recognizer serves the
    // first pointer family per interaction; splitting keeps this test on
    // the drag anchors.)
    let pad = PointerInteract()
        .on_pointer_down(m_down)
        .on_pointer_move(m_move)
        .on_pointer_up(m_up)
        .query_key("rut/pad")
        .child(Text().text("drag").build())
        .build();
    mount(Column().child(Text().text_bound(log).query_key("rut/text").build()).child(pad).build());
    return log.atom_id();
}
"#;

#[test]
fn f_drag_mutations_receive_typed_pointer_events() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_rut_module(DRAG_RUT).unwrap();
    app.wait_for_timeout(Duration::ZERO);

    let pad = app.query_element(&["rut", "pad"]).expect("pad not found");
    let pad_node = app
        .dev_tool_get_element(tur_engine::core::element::NodeId::from(
            tur_engine::core::element::ElementNodeId::new(pad.as_u64()),
        ))
        .unwrap();
    let origin = pad_node.absolute;

    // Down + move + up: the globals are the canvas coordinates; the move's
    // global-minus-local pins the pad origin (the anchor math).
    let (dx, dy) = (origin.0 + 10.0, origin.1 + 6.0);

    app.pointer_down(dx, dy);
    app.pointer_move(dx + 5.0, dy + 3.0);
    app.pointer_up(dx + 5.0, dy + 3.0);
    app.wait_for_timeout(Duration::ZERO);

    let transcript = rut_bound_text(&app);
    let (ix, iy) = (dx as u64, dy as u64);
    assert!(
        transcript.contains(&format!("|d{ix},{iy}")),
        "the down's global anchor landed: {transcript}"
    );
    assert!(
        transcript.contains(&format!("|m{}", origin.0 as u64))
            && transcript.contains(&format!(",{}", origin.1 as u64)),
        "the move's global-minus-local landed the pad origin: {transcript}"
    );
    assert!(transcript.contains("|ul"), "the up's button decodes primary: {transcript}");
}

// ---------------------------------------------------------------------------
// (g) — no remounts: a click's mutation pass re-renders the bound label in
// place; every node id in the tree stays put.
// ---------------------------------------------------------------------------

#[test]
fn g_clicks_never_remount_the_tree() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_rut_module(COUNTER_RUT).unwrap();
    app.wait_for_timeout(Duration::ZERO);

    fn snapshot(app: &TurTestApp) -> Vec<u64> {
        let mut ids = Vec::new();
        let mut stack = vec![app.dev_tool_element_tree().unwrap().children[0]];
        while let Some(id) = stack.pop() {
            ids.push(id.as_u64());
            if let Some(node) = app.dev_tool_get_element(id) {
                for child in node.children {
                    stack.push(child);
                }
            }
        }
        ids.sort();
        ids
    }

    let before = snapshot(&app);
    let (px, py) = element_center(&app, "pad");
    for _ in 0..3 {
        app.click(px, py);
        app.wait_for_timeout(Duration::ZERO);
    }
    let after = snapshot(&app);
    assert_eq!(before, after, "the mutation passes repainted in place — no remounts");
    assert_eq!(rut_bound_text(&app), "3 clicks", "the writes landed");
}

// ---------------------------------------------------------------------------
// The async launch rail: `spawn` + the task-scoped `TaskCtx` — reads and
// writes across an `await`, with the handles bound at the launch site.
// ---------------------------------------------------------------------------

const SPAWN_RUT: &str = r#"
use tur_host::{ clipboard_read, clipboard_write, spawn };
use tur_kit::{ Column, DeriveCtx, Mutation, MutationCtx, Readable, Source, TaskCtx, Text, derive, mount, mutate, source };

async fn work(ctx: TaskCtx, label: Readable<str>, busy: Readable<bool>) -> str {
    ctx.set<bool>(busy, true);
    let t = ctx.get<str>(label);
    await clipboard_write("from spawn");
    let clip = await clipboard_read();
    ctx.set<str>(label, f"{t}|{clip}");
    ctx.set<bool>(busy, false);
    return "";
}

entry fn start() -> u64 {
    let label: Readable<str> = source<str>("launched");
    let busy: Readable<bool> = source<bool>(false);
    spawn(work(TaskCtx.mint(), label, busy));
    let col = Column().child(Text().text_bound(label).query_key("rut/text").build());
    mount(col.build());
    return busy.atom_id();
}
"#;

#[test]
fn spawn_task_reads_and_writes_through_its_ctx_across_awaits() {
    let app = TurTestApp::new_with_http(400.0, 600.0).unwrap();
    app.set_clipboard_read("seeded");
    app.load_rut_module(SPAWN_RUT).unwrap();
    // The kit prelude compiles on the worker in real time; the awaits
    // resume on later pumps — poll on the real clock.
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    let done = loop {
        if rut_bound_text(&app) == "launched|seeded" {
            break true;
        }
        if std::time::Instant::now() > deadline {
            break false;
        }
        app.wait_for_timeout(Duration::from_millis(25));
    };
    assert!(
        done,
        "the task's ctx writes landed across the awaits: {:?}",
        rut_bound_text(&app)
    );
}

// ---------------------------------------------------------------------------
// (h) — the GENERIC surface: `source<T>` / `derive<T>` / `mutate<A>` +
// `ctx.get<T>` / `ctx.set<T>` / `ctx.run<A>` — angle-bracket type args,
// never per-type mint or method names (the user-settled surface law).
// ---------------------------------------------------------------------------

const GENERIC_RUT: &str = r#"

use tur_kit::{ Column, DeriveCtx, Mutation, MutationCtx, PointerInteract, Readable, Source, TaskCtx, Text, derive, mount, mutate, source };

entry fn start() -> u64 {
    let n = source<f64>(0.0);
    let tag = source<str>("");
    let label = derive<str>(fn (ctx: DeriveCtx) -> str {
        return f"{ctx.get<f64>(n) as u64} clicks";
    });
    let add_tag = mutate<f64>(fn (ctx: MutationCtx, x: f64) {
        ctx.set<str>(tag, f"step {x as u64}");
    });
    let fire = mutate(fn (ctx: MutationCtx, _e: nil) {
        ctx.set<f64>(n, ctx.get<f64>(n) + 1.0);
        ctx.run<f64>(add_tag, 2.0);
    });
    let col = Column()
        .child(Text().text_bound(label).query_key("rut/text").build())
        .child(Text().text_bound(tag).query_key("rut/tag").build())
        .child(PointerInteract().on_click(fire).query_key("rut/pad").child(Text().text("tap").build()).build());
    mount(col.build());
    return n.atom_id();
}
"#;

#[test]
fn h_generic_ctx_spelling_runs_the_triad() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_rut_module(GENERIC_RUT).unwrap();
    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(rut_bound_text(&app), "0 clicks");

    let (px, py) = element_center(&app, "pad");
    app.click(px, py);
    app.wait_for_timeout(Duration::ZERO);
    assert_eq!(
        rut_bound_text(&app),
        "1 clicks",
        "ctx.set<f64> wrote through the generic method"
    );
    assert_eq!(
        rut_text_at(&app, "tag"),
        "step 2",
        "ctx.run<f64> composed a typed-arg mutation"
    );
}
