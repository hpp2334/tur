//! JS-facing image-resource handle + prop resolution.
//!
//! [`ImageResourceHandle`] is the JS-opaque handle type — minted by
//! `createImageResource` / `createSvgResource` (worker-decoded) and wrapped
//! from a host-delivered numeric id by `imageResourceHandle` (host-side
//! registration via `TurApp::register_image`). Same pattern as the virtual
//! app's `ModuleSourceHandle`: a `JsData` payload downcast without the
//! runtime, unforgeable from JS (the numeric id alone is just a number —
//! the handle type is the typed surface).
//!
//! [`ImageResourceRef`] is the prop-side union: `resourceId` accepts either
//! a plain number (legacy hand-written ids / numeric atoms) or a handle.

use boa_engine::{Context, JsError, JsValue};
use boa_gc::{Finalize, Trace};

use crate::core::image_resource::ImageResourceId;
use crate::core::js_runtime::js_value::{IntoJs, type_error};

/// JS-opaque handle to a registered image resource — wraps the engine-side
/// [`ImageResourceId`]. The numeric id crosses embedder rails (event bus,
/// interpolated module source) as a plain number; this type is what JS code
/// passes around.
#[derive(Debug, Trace, Finalize, boa_engine::JsData)]
#[boa_gc(unsafe_empty_trace)]
pub(crate) struct ImageResourceHandle(pub(crate) ImageResourceId);

impl IntoJs for ImageResourceHandle {
    fn into_js(self, ctx: &mut Context) -> JsValue {
        let proto = ctx.intrinsics().constructors().object().prototype();
        boa_engine::JsObject::from_proto_and_data(proto, self).into()
    }
}

/// `resourceId` prop value: a numeric id or an [`ImageResourceHandle`] —
/// resolved to the engine-side id. The number path keeps hand-written ids
/// and numeric reactive atoms working (back-compat with the days when
/// `createImageResource` returned a bare number).
#[derive(Debug, Clone, Copy)]
pub(crate) struct ImageResourceRef(pub(crate) u64);

impl crate::core::js_runtime::js_value::FromJs for ImageResourceRef {
    fn from_js(v: &JsValue) -> Result<Self, JsError> {
        if let Some(n) = v.as_number() {
            return Ok(ImageResourceRef(n as u64));
        }
        if let Some(obj) = v.as_object()
            && let Some(handle) = obj.downcast_ref::<ImageResourceHandle>()
        {
            return Ok(ImageResourceRef(handle.0.as_u64()));
        }
        Err(type_error(
            "an image resource id (number) or an ImageResourceHandle \
             (from createImageResource / createSvgResource / imageResourceHandle)",
        ))
    }
}
