#![allow(dead_code)]

//! Image-resource prop resolution.
//!
//! [`ImageResourceRef`] is the `resourceId` prop type: the numeric
//! engine-side id (`ImageResourceId`), decoded from a native [`Value`]
//! number (the `img_res_bytes` / `img_res_solid` rows return the id as a
//! `u64`).

use crate::core::edgy::value::{Value, type_error};
use crate::core::image_resource::ImageResourceId;

/// `resourceId` prop value: the numeric engine-side resource id. Wraps
/// [`ImageResourceId`] so the element's reactive prop decodes through the
/// ordinary [`FromValue`](crate::core::edgy::FromValue) rail.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ImageResourceRef(pub(crate) u64);

impl ImageResourceRef {
    /// The engine-side resource id.
    pub(crate) fn id(&self) -> ImageResourceId {
        ImageResourceId::new(self.0)
    }
}

impl crate::core::edgy::FromValue for ImageResourceRef {
    fn from_value(v: &Value) -> Result<Self, String> {
        if let Some(n) = v.as_num() {
            return Ok(ImageResourceRef(n as u64));
        }
        Err(type_error(
            "an image resource id (number, from img_res_bytes / img_res_solid)",
        ))
    }
}
