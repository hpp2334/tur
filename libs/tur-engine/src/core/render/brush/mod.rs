//! Color/Brush domain — value types.
//!
//! Owns the engine's color primitives (`Color`, `Brush`, `GradientStop`,
//! `RGB`). The opaque wrapper handles (`ColorOpaque`, `BrushOpaque`) —
//! how reactive props and rut rows carry color/brush values by identity —
//! live in [`opaque`].
//!
//! Lives under `core::render` because painting is the primary consumer of
//! color/brush values; `Canvas::fill_*` and the vello renderer read from here.

pub mod color;
pub mod opaque;

pub use color::{Brush, Color, GradientStop, RGB};
