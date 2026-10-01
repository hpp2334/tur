//! The image family's `tur` host-pkg rows (via the pkg-extension seam):
//! byte-resource registration + the Image element spec.
//!
//! - `img_res_bytes(bytes) -> u64` — decode + register (the
//!   `createImageResource` twin: same worker-side decode, same
//!   upload-to-host rail via `register_image`).
//! - `img_res_solid(w, h, color) -> u64` — a synthetic RGBA resource (the
//!   rut twin of embedding PNG bytes in a fixture).

use std::rc::Rc;

use crate::builtin_plugins::image::decode::decode_image_bytes;
use crate::builtin_plugins::image::element::ImageView;
use crate::builtin_plugins::image::handle::ImageResourceRef;
use crate::core::image_resource::ImageResource;
use crate::core::layout::BoxFit;
use crate::core::rut_runtime::{RutHandles, RutView};
use crate::core::view::Val;
use num_traits::FromPrimitive;

use rut_vm::Opaque;

/// The pkg-extension payload: decl rows at compile time, bodies at boot.
pub(crate) fn install_ext(cx: &mut crate::core::rut_runtime::RutPkgCx<'_>) {
    install_decl(cx);
    let Some(handles) = cx.handles else {
        return; // the compile-time decl probe — bodies install at boot only
    };
    let handles = handles.clone();
    install(&mut *cx.pkg, &handles);
}

/// Declare the image rows + `BoxFit` consts (the installer's decl half).
pub fn install_decl(cx: &mut crate::core::rut_runtime::RutPkgCx<'_>) {
    let row = |n: &str, p: Vec<rut_core::types::TypeId>, r: rut_core::types::TypeId| {
        (n.to_string(), p, r, false)
    };
    cx.decl.extend(vec![
        row("img_res_bytes", vec![TY_BYTES], TY_U64),
        row("img_res_solid", vec![TY_U64, TY_U64, TY_U64], TY_U64),
        row("img_new", vec![TY_U64], TY_OPAQUE),
        row("img_width", vec![TY_OPAQUE, TY_F64], TY_NIL),
        row("img_height", vec![TY_OPAQUE, TY_F64], TY_NIL),
        row("img_fit", vec![TY_OPAQUE, TY_U64], TY_NIL),
        row("img_qkey", vec![TY_OPAQUE, TY_STR], TY_NIL),
        row("img_build", vec![TY_OPAQUE], TY_OPAQUE),
    ]);
    let c = |name: &str, v: u64| (name.to_string(), TY_U64, v);
    // The `BoxFit` flag consts (the `BoxFit` `FromPrimitive` order).
    cx.consts.extend(vec![
        c("BOXFIT_FILL", 0),
        c("BOXFIT_CONTAIN", 1),
        c("BOXFIT_COVER", 2),
        c("BOXFIT_FIT_WIDTH", 3),
        c("BOXFIT_FIT_HEIGHT", 4),
        c("BOXFIT_NONE", 5),
    ]);
}

use rut_core::types::{TY_BYTES, TY_F64, TY_NIL, TY_OPAQUE, TY_STR, TY_U64};

/// The image spec.
pub(crate) struct ImgSpec {
    resource_id: Option<Val<ImageResourceRef>>,
    width: Option<Val<f64>>,
    height: Option<Val<f64>>,
    fit: Option<Val<BoxFit>>,
    query_key: Option<Vec<String>>,
}

fn box_fit_of(v: u64) -> BoxFit {
    BoxFit::from_u64(v).unwrap_or(BoxFit::Contain)
}

/// Install the image-row bodies (the installer's boot half).
pub fn install(pkg: &mut rut_vm::interp::HostPkg, handles: &Rc<RutHandles>) {
    // Decode + register an encoded image (PNG/JPEG) — the same decode +
    // upload rail the JS `createImageResource` bridge rides.
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "img_res_bytes", (&[u8],) -> u64, move |_vm: &mut rut_vm::interp::Vm, bytes: &[u8]| {
        let image = decode_image_bytes(bytes)
            .ok_or_else(|| rut_vm::Trap::new(rut_vm::TrapKind::Invalid, "img_res_bytes: decode failed (supported: PNG, JPEG)"))?;
        Ok(h.inst.register_image(image).as_u64())
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
        Ok(h.inst.register_image(image).as_u64())
    });

    // ---- the image spec family ------------------------------------------
    rut_vm::pkg_fn!(pkg, "img_new", (u64,) -> rut_vm::OpaqueRef, |vm: &mut rut_vm::interp::Vm, res: u64| {
        let spec = ImgSpec {
            resource_id: Some(Val::Static(ImageResourceRef(res))),
            width: None,
            height: None,
            fit: None,
            query_key: None,
        };
        Ok(Opaque::alloc(vm, spec)?.handle().clone())
    });
    rut_vm::pkg_fn!(pkg, "img_width", (Opaque<ImgSpec>, f64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<ImgSpec>, v: f64| {
        b.with_mut(vm, |_vm, s| s.width = (v > 0.0).then_some(Val::Static(v)))
    });
    rut_vm::pkg_fn!(pkg, "img_height", (Opaque<ImgSpec>, f64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<ImgSpec>, v: f64| {
        b.with_mut(vm, |_vm, s| s.height = (v > 0.0).then_some(Val::Static(v)))
    });
    rut_vm::pkg_fn!(pkg, "img_fit", (Opaque<ImgSpec>, u64) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<ImgSpec>, v: u64| {
        b.with_mut(vm, |_vm, s| s.fit = Some(Val::Static(box_fit_of(v))))
    });
    rut_vm::pkg_fn!(pkg, "img_qkey", (Opaque<ImgSpec>, &str) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<ImgSpec>, key: &str| {
        let key: Vec<String> = key.split('/').map(str::to_string).collect();
        b.with_mut(vm, |_vm, s| s.query_key = Some(key))
    });
    rut_vm::pkg_fn!(pkg, "img_build", (Opaque<ImgSpec>,) -> rut_vm::OpaqueRef, |vm: &mut rut_vm::interp::Vm, b: Opaque<ImgSpec>| {
        let view = b.with(|s| {
            Rc::new(ImageView {
                resource_id: s.resource_id.clone(),
                width: s.width.clone(),
                height: s.height.clone(),
                fit: s.fit.clone(),
                query_key: s.query_key.clone(),
                child: None,
            }) as Rc<dyn crate::core::view::View>
        })?;
        Ok(Opaque::alloc(vm, RutView(view))?.handle().clone())
    });
}
