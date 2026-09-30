//! Phase-4 corpus rows — `Image`: byte-resource registration + the image
//! element.
//!
//! - `img_res_bytes(bytes) -> u64` — decode + register (the
//!   `createImageResource` twin: same worker-side decode, same
//!   upload-to-host rail via `register_image`).
//! - `img_res_solid(w, h, color) -> u64` — a synthetic RGBA resource (the
//!   rut twin of embedding PNG bytes in a fixture).
//! - `el_image(res, w, h, fit)` — the `Image` element.

use std::rc::Rc;

use crate::builtin_plugins::image::element::ImageView;
use crate::builtin_plugins::image::handle::ImageResourceRef;
use crate::core::image_resource::ImageResource;
use crate::core::layout::BoxFit;
use crate::core::view::Val;
use num_traits::FromPrimitive;

use rut_vm::Opaque;

use super::{RutView, RutHandles};

pub fn decl_rows() -> Vec<(String, Vec<rut_core::types::TypeId>, rut_core::types::TypeId)> {
    use rut_core::types::*;
    vec![
        ("img_res_bytes", vec![TY_BYTES], TY_U64),
        ("img_res_solid", vec![TY_U64, TY_U64, TY_U64], TY_U64),
        (
            "el_image",
            vec![TY_U64, TY_F64, TY_F64, TY_U64],
            TY_OPAQUE,
        ),
    ]
    .into_iter()
    .map(|(n, p, r)| (n.to_string(), p, r))
    .collect()
}

/// The `BoxFit` flag consts (the `BoxFit` `FromPrimitive` order).
pub fn decl_consts() -> Vec<(String, rut_core::types::TypeId, u64)> {
    use rut_core::types::TY_U64;
    let c = |name: &str, v: u64| (name.to_string(), TY_U64, v);
    vec![
        c("BOXFIT_FILL", 0),
        c("BOXFIT_CONTAIN", 1),
        c("BOXFIT_COVER", 2),
        c("BOXFIT_FIT_WIDTH", 3),
        c("BOXFIT_FIT_HEIGHT", 4),
        c("BOXFIT_NONE", 5),
    ]
}

fn box_fit_of(v: u64) -> BoxFit {
    BoxFit::from_u64(v).unwrap_or(BoxFit::Contain)
}

pub fn install(pkg: &mut rut_vm::interp::HostPkg, handles: &Rc<RutHandles>) {
    // Decode + register an encoded image (PNG/JPEG) — the same decode +
    // upload rail the JS `createImageResource` bridge rides.
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "img_res_bytes", (&[u8],) -> u64, move |_vm: &mut rut_vm::interp::Vm, bytes: &[u8]| {
        let image = crate::builtin_plugins::image::decode::decode_image_bytes(bytes)
            .ok_or_else(|| rut_vm::Trap::new(rut_vm::TrapKind::Invalid, "img_res_bytes: decode failed (supported: PNG, JPEG)"))?;
        Ok(h.js_ctx.register_image(image).as_u64())
    });

    // A solid w×h RGBA resource (fixture-friendly synthetic image).
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "img_res_solid", (u64, u64, u64) -> u64, move |_vm: &mut rut_vm::interp::Vm, w: u64, hh: u64, color: u64| {
        let (r, g, b, a) = (
            ((color >> 24) & 0xFF) as u8,
            ((color >> 16) & 0xFF) as u8,
            ((color >> 8) & 0xFF) as u8,
            (color & 0xFF) as u8,
        );
        let (w, hh) = (w.max(1) as usize, hh.max(1) as usize);
        let mut rgba = Vec::with_capacity(w * hh * 4);
        for _ in 0..w * hh {
            rgba.extend_from_slice(&[r, g, b, a]);
        }
        let image = ImageResource::from_rgba(&rgba, w as u32, hh as u32)
            .ok_or_else(|| rut_vm::Trap::new(rut_vm::TrapKind::Invalid, "img_res_solid: bad geometry"))?;
        Ok(h.js_ctx.register_image(image).as_u64())
    });

    // The image element: (resource_id, width, height, fit).
    rut_vm::pkg_fn!(pkg, "el_image", (u64, f64, f64, u64) -> rut_vm::OpaqueRef, |vm: &mut rut_vm::interp::Vm, res: u64, w: f64, hh: f64, fit: u64| {
        let view = Rc::new(ImageView {
            resource_id: Some(Val::Static(ImageResourceRef(res))),
            width: (w > 0.0).then_some(Val::Static(w)),
            height: (hh > 0.0).then_some(Val::Static(hh)),
            fit: Some(Val::Static(box_fit_of(fit))),
            query_key: None,
            child: None,
        });
        Ok(Opaque::alloc(vm, RutView(view))?.handle().clone())
    });
}

