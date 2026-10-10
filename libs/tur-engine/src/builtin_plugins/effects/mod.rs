//! Visual-effect elements — `Opacity` (alpha-mask a child subtree) and
//! `Transform` (2D affine rotate / scale / translate). These are pure visual
//! effects, not animation; the animation machinery (`AnimationController`,
//! `Tween`, implicit-animation widgets) lives in the separate `tur-animation`
//! crate.

mod element;
mod layout;
mod render;

pub use element::{OpacityElement, OpacityView, TransformElement, TransformView};
