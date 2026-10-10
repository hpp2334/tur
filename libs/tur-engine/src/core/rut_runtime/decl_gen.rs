//! The `tur_host` decl-surface generator: the merged pkg surface
//! ([`tur_decl_pkg`] + every extension's rows, the exact assembly a boot
//! compiles against) rendered as `.d.rut` decl text.
//!
//! The Rust rows stay the compile truth; the rendered text is the
//! committed, test-pinned snapshot at `rut/tur_host/tur_host.d.rut` (the
//! pkg's `entry.type` per its `rut.jsonc`). The pin test
//! (`tur_host_decl_snapshot` in tur-integration-tests) rebuilds the text
//! from the standard session and diffs it — a Rust row that drifts from
//! the committed snapshot fails the suite until the snapshot is
//! deliberately regenerated (the test's bless mode).

use rut_core::types::{
    TypeId, TY_BOOL, TY_BYTES, TY_F32, TY_F64, TY_I16, TY_I32, TY_I64, TY_I8, TY_NIL, TY_OPAQUE,
    TY_OPT_BYTES, TY_OPT_OPAQUE, TY_OPT_STR, TY_STR, TY_U16, TY_U32, TY_U64, TY_U8,
};

use super::{tur_decl_pkg, RutPkgCx, RutPkgExt};
use rut_driver::PkgBody;

/// A `host fn` row's surface shape — the decl pkg's tuple, named.
pub type HostRow = (String, Vec<TypeId>, TypeId, bool);

/// The crossing-set type name for a decl signature (`decl.rs`'s table, in
/// reverse: the engine rows only ever use these).
fn ty_name(t: TypeId) -> &'static str {
    match t {
        TY_NIL => "nil",
        TY_BOOL => "bool",
        TY_STR => "str",
        TY_BYTES => "bytes",
        TY_F32 => "f32",
        TY_F64 => "f64",
        TY_I8 => "i8",
        TY_I16 => "i16",
        TY_I32 => "i32",
        TY_I64 => "i64",
        TY_U8 => "u8",
        TY_U16 => "u16",
        TY_U32 => "u32",
        TY_U64 => "u64",
        TY_OPAQUE => "opaque",
        TY_OPT_STR => "?str",
        TY_OPT_BYTES => "?bytes",
        TY_OPT_OPAQUE => "?opaque",
        _ => "opaque",
    }
}

/// The merged `tur_host` surface for a session: the engine rows plus every
/// extension's decl rows + consts, in install order (the exts run against
/// a compile-time probe context — `handles: None`, exactly like
/// `RutRuntime::compile`'s assembly). `(host fns, consts)`.
pub fn tur_host_surface(exts: &[RutPkgExt]) -> (Vec<HostRow>, Vec<(String, TypeId, u64)>) {
    let mut tur_pkg = tur_decl_pkg();
    let mut ext_decl: Vec<HostRow> = Vec::new();
    let mut ext_consts: Vec<(String, TypeId, u64)> = Vec::new();
    let mut consts_out: Vec<(String, TypeId, u64)> = Vec::new();
    let mut pkg = rut_vm::interp::HostPkg::new("tur_host");
    let mut preludes: Vec<rut_driver::Pkg> = Vec::new();
    for ext in exts {
        ext(&mut RutPkgCx {
            decl: &mut ext_decl,
            consts: &mut ext_consts,
            pkg: &mut pkg,
            handles: None,
            preludes: &mut preludes,
        });
    }
    if let PkgBody::Host {
        host_funcs, consts, ..
    } = &mut tur_pkg.body
    {
        host_funcs.extend(ext_decl);
        consts.extend(ext_consts);
        consts_out = consts.clone();
    }
    // The host fns out of the assembled pkg (the exts' rows appended).
    let funcs = match &tur_pkg.body {
        PkgBody::Host { host_funcs, .. } => host_funcs.clone(),
        _ => Vec::new(),
    };
    (funcs, consts_out)
}

/// Render the merged surface as the COMPLETE `tur_host.d.rut` snapshot
/// text — the file header (the generated-artifact note + the regenerate
/// instructions) then one `pub host fn` row per crossing, positional param
/// names (the engine rows carry no param names; the crossing is positional
/// too). Async rows render `async` (the same lowering `decl.rs` reads
/// back).
pub fn render_tur_host_decl(exts: &[RutPkgExt]) -> String {
    let (funcs, consts) = tur_host_surface(exts);
    let mut out = String::new();
    out.push_str(
        "// tur_host.d.rut — GENERATED from the engine's `tur_host` rows; do not\n\
         // edit by hand. The Rust rows are the compile truth\n\
         // (tur-engine/src/core/rut_runtime: `tur_decl_pkg()` + every plugin's\n\
         // RutPkgExt rows, the bodies installed by `install_tur_pkg`); this file\n\
         // is the test-pinned SNAPSHOT of that surface (the pkg's `entry.type`\n\
         // per rut.jsonc). Regenerate with the pin test's bless mode:\n\
         //   TUR_BLESS_TUR_HOST_DECL=1 cargo nextest run -p tur-integration-tests\n\
         //     tur_host_decl\n\
         // Param names are positional (p1..pn) — the engine rows carry none; the\n\
         // crossing is positional too.\n\n",
    );
    for (name, params, ret, is_async) in &funcs {
        let params = params
            .iter()
            .enumerate()
            .map(|(i, t)| format!("p{}: {}", i + 1, ty_name(*t)))
            .collect::<Vec<_>>()
            .join(", ");
        let ret = if *ret == TY_NIL {
            String::new()
        } else {
            format!(" -> {}", ty_name(*ret))
        };
        let async_kw = if *is_async { "async " } else { "" };
        out.push_str(&format!("pub host {async_kw}fn {name}({params}){ret};\n"));
    }
    // The surface grammar has no const spelling (the manifest's `consts`
    // table is their only form); the standard session carries none since
    // the flag consts died — the pin test asserts that stays true.
    if !consts.is_empty() {
        let names = consts
            .iter()
            .map(|(n, _, _)| n.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!(
            "// ({} const rows carry no surface spelling — see the manifest's consts table)\n",
            names
        ));
    }
    out
}