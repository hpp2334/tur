//! Color/Brush domain — value types + JS bindings.
//!
//! Owns the engine's color primitives (`Color`, `Brush`, `GradientStop`,
//! `RGB`) and the JS-exposed bindings (`Color` / `LinearGradient` const-
//! objects, `createColor` / `colorLerp` / `createLinearGradient` bridge fns).
//! The opaque wrappers (`ColorOpaque`, `BrushOpaque`) and the `FromJs`
//! impls for `Color` / `Brush` live in [`opaque`].
//!
//! Lives under `core::render` because painting is the primary consumer of
//! color/brush values; `Canvas::fill_*` and the vello renderer read from here.

pub mod bridge;
pub mod color;
pub mod opaque;

pub use color::{Brush, Color, GradientStop, RGB};
pub use opaque::{BrushOpaque, ColorOpaque};
