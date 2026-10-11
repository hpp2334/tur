//! The `tur_host` decl-surface generator: the merged pkg surface
//! ([`tur_decl_pkg`] + every extension's rows, the exact assembly a boot
//! compiles against) rendered as the pkg's DECL MODULE TREE — a
//! decls-organized `mod.d.rut` root + one file per module under it.
//!
//! The Rust rows stay the compile truth; the rendered tree is the
//! committed, test-pinned snapshot at `rut/tur_host/` (the pkg is
//! `type = "host"`; `mod.d.rut` beside its `rut.jsonc` is the
//! auto-discovered root — no `entry` is spelled). The pin test
//! (`tur_host_decl` in tur-integration-tests) rebuilds the file set
//! from the standard session and diffs it — a Rust row that drifts
//! from the committed tree fails the suite until the snapshot is
//! deliberately regenerated (the test's bless mode).

use std::collections::BTreeMap;

use rut_ast::ast::Vis;
use rut_core::types::{
    TypeId, TY_BOOL, TY_BYTES, TY_F32, TY_F64, TY_I16, TY_I32, TY_I64, TY_I8, TY_NIL, TY_OPAQUE,
    TY_OPT_BYTES, TY_OPT_OPAQUE, TY_OPT_STR, TY_STR, TY_U16, TY_U32, TY_U64, TY_U8,
};

use super::{tur_decl_pkg, RutPkgCx, RutPkgExt};
use rut_driver::{HostRow, ModSource, PkgBody};

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

/// One `pub host fn` decl line — positional param names (the engine rows
/// carry none; the crossing is positional too). Async rows render
/// `async` (the same lowering `decl.rs` reads back).
fn row_line(f: &HostRow) -> String {
    let params = f
        .params
        .iter()
        .enumerate()
        .map(|(i, t)| format!("p{}: {}", i + 1, ty_name(*t)))
        .collect::<Vec<_>>()
        .join(", ");
    let ret = if f.ret == TY_NIL {
        String::new()
    } else {
        format!(" -> {}", ty_name(f.ret))
    };
    let async_kw = if f.is_async { "async " } else { "" };
    format!("pub host {async_kw}fn {}({params}){ret};\n", f.name)
}

/// The root file's header — the generated-artifact note, the bless
/// instructions, and the entry law (the `mod.d.rut` root is
/// auto-discovered; no `entry` is spelled in the manifest).
const ROOT_HEADER: &str = "// tur_host/mod.d.rut — GENERATED from the engine's `tur_host` rows; do\n\
// not edit by hand. The Rust rows are the compile truth\n\
// (tur-engine/src/core/rut_runtime: `tur_decl_pkg()` + every plugin's\n\
// RutPkgExt rows, the bodies installed by `install_tur_pkg`); this tree\n\
// is the test-pinned SNAPSHOT of that surface (a `type = \"host\"` pkg —\n\
// `mod.d.rut` beside rut.jsonc is the auto-discovered root; the manifest\n\
// spells no `entry`). Regenerate with the pin test's bless mode:\n\
//   TUR_BLESS_TUR_HOST_DECL=1 cargo nextest run -p tur-integration-tests\n\
//     tur_host_decl\n\
// Param names are positional (p1..pn) — the engine rows carry none; the\n\
// crossing is positional too. The surface organizes as a MODULE TREE:\n\
// each `pub mod X;` mounts `X/mod.d.rut` beside this file, and a\n\
// consumer's use path spells the tree\n\
// (`use tur_host::store::{ rs_source_str };`; root rows stay flat —\n\
// `use tur_host::{ mount };`).\n\n";

/// The rows grouped by module path (the root first, install order kept
/// within a module) with every intermediate prefix materialized —
/// `layout/flex` contributes the `layout` group even when no row lives
/// there. The tree shape both the file renderer and the in-memory
/// mount build from.
fn module_groups(funcs: &[HostRow]) -> BTreeMap<String, Vec<&HostRow>> {
    let mut modules: BTreeMap<String, Vec<&HostRow>> = BTreeMap::new();
    for f in funcs {
        modules.entry(f.path.clone()).or_default().push(f);
    }
    let mut prefixes: Vec<String> = Vec::new();
    for path in modules.keys() {
        let mut at = String::new();
        for seg in path.split('/') {
            if !at.is_empty() {
                at.push('/');
            }
            at.push_str(seg);
            if !modules.contains_key(&at) && !prefixes.contains(&at) {
                prefixes.push(at.clone());
            }
        }
    }
    for p in prefixes {
        modules.entry(p).or_default();
    }
    modules
}

/// A module's children — the next-segment mod names under `path`,
/// sorted (`layout` → box, flex, flexi, grid_table, pos, stack).
fn child_mods(modules: &BTreeMap<String, Vec<&HostRow>>, path: &str) -> Vec<String> {
    let prefix = if path.is_empty() {
        String::new()
    } else {
        format!("{path}/")
    };
    let mut kids: Vec<String> = modules
        .keys()
        .filter(|p| {
            p.strip_prefix(&prefix)
                .map(|rest| !rest.is_empty() && !rest.contains('/'))
                .unwrap_or(false)
        })
        .cloned()
        .collect();
    kids.sort();
    kids
}

/// The decl text for one module file: the `pub mod` edges then the
/// rows (the exact shape the generated `.d.rut` files carry, minus
/// the prose headers).
fn module_text(
    modules: &BTreeMap<String, Vec<&HostRow>>,
    path: &str,
    rows: &[&HostRow],
) -> String {
    let mut text = String::new();
    let kids = child_mods(modules, path);
    for kid in &kids {
        let leaf = kid.rsplit('/').next().unwrap_or(kid);
        text.push_str(&format!("pub mod {leaf};\n"));
    }
    if !kids.is_empty() && !rows.is_empty() {
        text.push('\n');
    }
    for row in rows {
        text.push_str(&row_line(row));
    }
    text
}

/// Mount the rows' module tree onto a hand-built host pkg — the
/// in-memory twin of the loader's directory walk: the root surface
/// text (the `pub mod` decls + the root rows) and one `ModSource` per
/// module (intermediates included, all `pub`). The qualified-use walk
/// (`use tur_host::store::{ rs_source_str };`) reads THIS tree — a
/// hand-built pkg without it is flat by the compiler's law. Idempotent:
/// re-run after extending the row set (the exts' families).
pub fn mount_host_tree(pkg: &mut rut_driver::Pkg) {
    let rows = match &pkg.body {
        PkgBody::Host { host_funcs, .. } => host_funcs.clone(),
        _ => return,
    };
    let groups = module_groups(&rows);
    let mut mods = BTreeMap::new();
    let mut root_text = String::new();
    for (path, group) in &groups {
        let text = module_text(&groups, path, group);
        if path.is_empty() {
            root_text = text;
        } else {
            mods.insert(
                path.clone(),
                ModSource { path: path.clone(), vis: Vis::Pub, text },
            );
        }
    }
    pkg.mods = mods;
    if let PkgBody::Host { root_surface, .. } = &mut pkg.body {
        *root_surface = Some(root_text);
    }
}

/// Render the merged surface as the COMPLETE decl-tree file set —
/// `(rel path, text)` pairs rooted at `rut/tur_host/`: the `mod.d.rut`
/// root (the header, the root rows, and the sorted `pub mod` decls)
/// plus one `NAME/mod.d.rut` per module — intermediate domain dirs
/// included (`layout/mod.d.rut` carries the `pub mod flex;` decls; the
/// rows live in `layout/flex/mod.d.rut`). Deterministic: modules sort by
/// path (the root first), rows keep install order within a module.
pub fn render_tur_host_decl_tree(exts: &[RutPkgExt]) -> Vec<(String, String)> {
    let (funcs, consts) = tur_host_surface(exts);
    // The surface grammar has no const spelling (the manifest's `consts`
    // table is their only form); the standard session carries none since
    // the flag consts died — the pin test asserts that stays true.
    assert!(
        consts.is_empty(),
        "the tur_host surface grew const rows ({:?}) — the decl-tree \
         grammar has no const spelling; give them a manifest consts table",
        consts.iter().map(|(n, _, _)| n).collect::<Vec<_>>()
    );

    let modules = module_groups(&funcs);
    let mut out: Vec<(String, String)> = Vec::new();
    for (path, rows) in &modules {
        let rel = if path.is_empty() {
            "mod.d.rut".to_string()
        } else {
            format!("{path}/mod.d.rut")
        };
        let mut text = String::new();
        if path.is_empty() {
            text.push_str(ROOT_HEADER);
        } else {
            text.push_str(&format!(
                "// tur_host/{path} — GENERATED (see the root mod.d.rut's \
                 header; do not edit by hand).\n\n"
            ));
        }
        text.push_str(&module_text(&modules, path, rows));
        out.push((rel, text));
    }
    out
}
