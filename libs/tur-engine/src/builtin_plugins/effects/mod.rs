//! Visual-effect elements — `Opacity` (alpha-mask a child subtree) and
//! `Transform` (2D affine rotate / scale / translate). These are pure visual
//! effects, not animation; the animation machinery (`AnimationController`,
//! `Tween`, implicit-animation widgets) lives in the separate `tur-animation`
//! crate.
//!
//! Installed into `tur:std` by [`install_effects`].

mod element;
mod layout;
mod render;
pub mod bridge;

pub use element::{OpacityElement, OpacityView, TransformElement, TransformView};

use crate::core::js_runtime::helpers::FnEntry;
use crate::core::plugin::PluginContext;
use crate::error::TurError;

/// Install the visual-effect elements (`Opacity`, `Transform`) and return
/// their JS factory fns to be merged into `tur:std` by the orchestrator.
pub fn install_effects(_ctx: &mut PluginContext<'_>) -> Result<Vec<FnEntry>, TurError> {
    Ok(bridge::fns())
}
