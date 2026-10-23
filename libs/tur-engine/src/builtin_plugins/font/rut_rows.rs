//! The lazy-font rows (via the pkg-extension seam): `font_declare` /
//! `font_declare_face` (sync metadata declarations) and `font_load_data`
//! (async — fetches the declared faces' bytes through the `Fonts`
//! capability's provider, registers them into the shared collection, and
//! bumps the epoch).
//!
//! The rows install only when the `Fonts` capability is present (the
//! `TurNetPlugin` pattern), so a plugin set without a provider never sees
//! (or compiles) the font kit.

use std::rc::Rc;

use rut_core::types::{TY_BOOL, TY_F64, TY_NIL, TY_STR};
use crate::core::fonts::{declared_face, FontFaceMeta};
use parley::FontStyle;

use super::Fonts;

/// The decl rows. `font_load_data` rides the driver's async family
/// expansion (`is_async = true`).
pub fn decl_rows() -> Vec<rut_driver::HostRow> {
    vec![
        ("font_declare", vec![TY_STR], TY_NIL, false),
        ("font_declare_face", vec![TY_STR, TY_F64, TY_BOOL], TY_NIL, false),
        ("font_load_data", vec![TY_STR], TY_BOOL, true),
    ]
    .into_iter()
    .map(|(n, p, r, a)| crate::core::rut_runtime::host_row_at("font", n, p, r, a))
    .collect()
}

fn meta_of(family: &str, weight: Option<f64>, italic: Option<bool>) -> FontFaceMeta {
    FontFaceMeta {
        family: family.to_string(),
        overridden_weight: weight.map(|w| w as u16),
        overridden_style: italic
            .map(|it| if it { FontStyle::Italic } else { FontStyle::Normal }),
        overridden_width: None,
    }
}

/// The `font_load_data` body — claims the family's pending faces, fetches
/// each through the provider, registers into the shared collection (epoch
/// bump per face), then dirties the tree so the epoch-keyed text memos
/// re-shape (the lazy-font swap).
fn font_load_data_body(
    h: Rc<crate::core::rut_runtime::RutHandles>,
    family: &str,
) -> rut_vm::Completer<bool> {
    let done = rut_vm::Completer::<bool>::new();
    let metas = h.font_rail.begin_load(family);
    if metas.is_empty() {
        // Nothing claimed: every declared face is already Loaded (true) or
        // the family was never declared (false).
        let ok = !h.font_rail.declared_faces(family).is_empty();
        done.complete(ok);
        return done;
    }
    let Some(fonts) = h.inst.capability().of::<Fonts>() else {
        // No provider: the claim dies with this call (nothing was Loading).
        done.complete(false);
        return done;
    };
    let provider = fonts.backend().clone();
    let rail = h.font_rail.clone();
    let tree = h.element_tree.clone();
    let request_frame = h.request_frame.clone();
    let dirty = h.dirty.clone();
    let family = family.to_string();
    let w = done.clone();
    h.inst.spawn_local(move |_aw| async move {
        let mut ok = true;
        for meta in &metas {
            match provider.load(meta).await {
                Ok(bytes) => {
                    ok &= rail.finish_load(meta, &bytes);
                }
                Err(e) => {
                    tracing::warn!("font_load_data({family}): the provider failed for {meta:?}: {e}");
                    ok = false;
                }
            }
        }
        // The registrations bumped the epoch — mark EVERY node layout-dirty
        // (the whole-tree relayout rail) so the next layout revisits each
        // element and the epoch-keyed text memos re-shape (a landed font
        // changes metrics), then wake for a frame.
        if ok {
            tree.mark_root_dirty();
        }
        dirty.set(true);
        request_frame();
        w.complete(ok);
    });
    done
}

/// (The allow covers the upstream `pkg_async_fn!` row expansion's cosmetic
/// — the spike's precedent.)
#[allow(clippy::needless_question_mark)]
pub fn install(cx: &mut crate::core::rut_runtime::RutPkgCx<'_>) {
    cx.decl.extend(decl_rows());
    let Some(handles) = cx.handles else {
        return;
    };
    let handles = handles.clone();
    let pkg = &mut *cx.pkg;

    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "font::font_declare", (&str,) -> (), move |_vm: &mut rut_vm::interp::Vm, family: &str| {
        h.font_rail.declare(declared_face(family.to_string()));
    });

    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "font::font_declare_face", (&str, f64, bool) -> (), move |_vm: &mut rut_vm::interp::Vm, family: &str, weight: f64, italic: bool| {
        h.font_rail.declare(meta_of(family, Some(weight), Some(italic)));
    });

    let h = handles.clone();
    rut_vm::pkg_async_fn!(pkg, "font::font_load_data", (&str,) -> bool, move |family: &str| {
        font_load_data_body(h.clone(), family)
    });
}
