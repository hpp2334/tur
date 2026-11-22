//! The embedder-supplied start-context gate: a module loaded WITH an
//! embedder start cx boots `entry fn start(cx: opaque)` with the identity
//! the embedder minted (sealed into a rut box), the cx entry lanes deliver
//! that handle — not the start answer — when addressed by the same id, the
//! str/bytes dispatch lanes round-trip, and the registry's file-set lane
//! loads a two-file module (diags still attribute to the child file).

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use tur_engine::ModuleSourceRegistry;
use tur_engine::core::app::RutEntryAnswer;
use tur_integration_tests::TurTestApp;

/// The boot shape: `start(cx)` unboxes the embedder identity, records it
/// into a bound Text (the observable), and answers its own u64 return
/// (42 — deliberately distinct from the identity so the "lanes deliver
/// the boot handle, not the start answer" law is provable). The atom
/// binding spells the concrete `Source<str>` — an interface-typed
/// (`Readable<str>`) local bound to Text inside an ENTRY fn body trips a
/// pre-existing interface-slot misbind at this rut pin (reproduces on the
/// unpatched engine; report material upstream).
const START_CX_RUT: &str = r#"
use tur_kit::handles::{ mount }; use tur_kit::reactive::{ Source, entry_ctx, source }; use tur_kit::text::core::{ Text };

entry fn start(cx: opaque) -> u64 {
    let id: u64 = unopaque<u64>(cx);
    let atom: Source<str> = source<str>("pending");
    entry_ctx().set<str>(atom, f"cx:{id}");
    mount(Text().text(atom).query_key("startcx/text").build());
    return 42;
}

entry fn probe(cx: opaque) -> u64 {
    return unopaque<u64>(cx);
}

entry fn probe_ctx(cx: opaque) -> opaque {
    return cx;
}
"#;

fn settle(app: &TurTestApp) {
    app.wait_for_timeout(Duration::ZERO);
}

#[test]
fn start_with_param_receives_the_embedder_arg() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_rut_module_cx(START_CX_RUT, 777).unwrap();
    settle(&app);

    // `start` unboxed the EMBEDDER's identity (777) — the atom write
    // recorded it — while its own return (42) is the boot answer.
    assert_eq!(
        app.query_text(&["startcx", "text"]).as_deref(),
        Some("cx:777"),
        "start received the embedder-supplied box"
    );
    assert_eq!(
        app.rut_start_answer(),
        42,
        "the start answer is start's return"
    );
}

#[test]
fn cx_lanes_deliver_the_boot_handle_not_the_start_answer() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_rut_module_cx(START_CX_RUT, 777).unwrap();
    settle(&app);

    // The lane addressed by the EMBEDDER id (777) delivers the boot box:
    // the probe unboxes 777 — never start's return (42).
    let a = app.call_rut_entry_cx_answer("probe", 777).unwrap();
    assert_eq!(
        a,
        RutEntryAnswer::U64(777),
        "the lane delivers the embedder-supplied handle"
    );

    // The identity token stays live across calls (one handle re-crosses
    // on every control call — the slot law).
    let a = app.call_rut_entry_cx_answer("probe", 777).unwrap();
    assert_eq!(a, RutEntryAnswer::U64(777));
}

#[test]
fn boot_handle_and_minted_tokens_never_collide() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_rut_module_cx(START_CX_RUT, 5).unwrap();
    settle(&app);

    // The boot box is registered under the embedder's id; an opaque
    // ANSWER mints its slot ABOVE it (the two token spaces never merge).
    let a = app.call_rut_entry_cx_answer("probe_ctx", 5).unwrap();
    let RutEntryAnswer::Opaque(token) = a else {
        panic!("probe_ctx answered the box itself, got {a:?}");
    };
    assert!(
        token > 5,
        "minted tokens climb above the boot identity (got {token})"
    );

    // Both resolve to the SAME handle: probing the boot id and the minted
    // token answer 5 / echo the box.
    let b = app.call_rut_entry_cx_answer("probe", 5).unwrap();
    assert_eq!(b, RutEntryAnswer::U64(5));
    let c = app.call_rut_entry_cx_answer("probe", token).unwrap();
    assert_eq!(c, RutEntryAnswer::U64(5));
}

#[test]
fn start_declaring_a_param_without_a_supplied_cx_is_a_loud_boot_error() {
    let app = TurTestApp::new(400.0, 600.0).unwrap();
    let err = app
        .load_rut_module(START_CX_RUT)
        .expect_err("the boot must refuse");
    assert!(
        err.to_string().contains("no start context was supplied"),
        "the error names the fix: {err}"
    );
}

/// The dispatch shapes: `entry fn(cx, op: str, args: str)` (the reply
/// envelope) and its bytes twin, plus a `-> bytes` return (the binary
/// answer lane). `start` takes NO param — the boot box is registered even
/// for a parameterless start (the lanes deliver it; the boot stays
/// zero-arg).
const DISPATCH_RUT: &str = r#"
entry fn start() {}

entry fn op_echo(cx: opaque, op: str, args: str) -> str {
    let _ = unopaque<u64>(cx);
    return f"{op}:{args}";
}

entry fn op_echo_bytes(cx: opaque, op: str, args: bytes) -> u64 {
    let _ = unopaque<u64>(cx);
    return args.len() as u64;
}

entry fn op_bytes_ret(cx: opaque, op: str, args: str) -> bytes {
    let _ = unopaque<u64>(cx);
    let _ = op;
    let _ = args;
    return bytes::from([9u8, 8u8]);
}
"#;

#[test]
fn str_dispatch_lane_roundtrips_the_payload() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_rut_module_cx(DISPATCH_RUT, 9).unwrap();
    settle(&app);

    let a = app
        .call_rut_entry_cx_str_str("op_echo", 9, "storage:list", "[1,2]")
        .unwrap();
    assert_eq!(
        a,
        RutEntryAnswer::Str("storage:list:[1,2]".to_string()),
        "op + args rode the call"
    );
}

#[test]
fn bytes_dispatch_lane_roundtrips_the_payload() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_rut_module_cx(DISPATCH_RUT, 9).unwrap();
    settle(&app);

    let a = app
        .call_rut_entry_cx_str_bytes("op_echo_bytes", 9, "bin", &[1u8, 2, 3])
        .unwrap();
    assert_eq!(a, RutEntryAnswer::U64(3), "the binary args crossed");

    // The `-> bytes` return decodes into the answer lane.
    let b = app
        .call_rut_entry_cx_str_str("op_bytes_ret", 9, "bin", "x")
        .unwrap();
    assert_eq!(b, RutEntryAnswer::Bytes(vec![9u8, 8]));
}

/// The registry's file-set lane: `register_files` + a two-file load — the
/// root's `mod child;` mounts the child file and its fn resolves
/// cross-module.
#[test]
fn file_set_registry_lane_loads_a_two_file_module() {
    let reg = ModuleSourceRegistry::new();
    let mut mods: BTreeMap<Arc<str>, Arc<str>> = BTreeMap::new();
    mods.insert(
        Arc::from("child"),
        Arc::from("pub fn label() -> str { return \"two-file\"; }"),
    );
    let root = "mod child;\nuse tur_kit::handles::{ mount }; use tur_kit::reactive::{ sv }; use tur_kit::text::core::{ Text };\nentry fn start() {\n    mount(Text().text(sv(child::label())).query_key(\"files/text\").build());\n}\n";
    let handle = reg.register_files(root, mods);

    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_rut_module_source(&reg, handle).unwrap();
    settle(&app);
    assert_eq!(
        app.query_text(&["files", "text"]).as_deref(),
        Some("two-file"),
        "the child file loaded and its fn resolved"
    );
    assert!(reg.remove(handle));
}

/// A broken CHILD file's diags attribute to the child (`<path>.rut`) —
/// the concatenated-unit byte layout the file set's BTreeMap order
/// defines.
#[test]
fn file_set_diags_attribute_to_the_child_file() {
    let reg = ModuleSourceRegistry::new();
    let mut mods: BTreeMap<Arc<str>, Arc<str>> = BTreeMap::new();
    mods.insert(
        Arc::from("child"),
        Arc::from("pub fn broken() -> str { let x: i32 = ; }"),
    );
    let root = "mod child;\nentry fn start() {}\n";
    let handle = reg.register_files(root, mods);

    let app = TurTestApp::new(400.0, 600.0).unwrap();
    let err = app
        .load_rut_module_source(&reg, handle)
        .expect_err("the broken child must refuse the load");
    assert!(
        err.to_string().contains("child.rut:"),
        "the diag names the child file: {err}"
    );
}

/// A file-set handle ALSO takes an embedder start cx (`_cx` twin) — the
/// multi-file rail and the start-cx crossing compose.
#[test]
fn file_set_lane_takes_the_embedder_start_cx() {
    let reg = ModuleSourceRegistry::new();
    let mods: BTreeMap<Arc<str>, Arc<str>> = BTreeMap::new();
    let root = "entry fn start(cx: opaque) -> u64 {\n    return unopaque<u64>(cx);\n}\nentry fn probe(cx: opaque) -> u64 {\n    return unopaque<u64>(cx);\n}\n";
    let handle = reg.register_files(root, mods);

    let mut app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_rut_module_source_cx(&reg, handle, 321).unwrap();
    settle(&app);
    assert_eq!(app.rut_start_answer(), 321, "start unboxed the identity");
    let a = app.call_rut_entry_cx_answer("probe", 321).unwrap();
    assert_eq!(a, RutEntryAnswer::U64(321));
}
