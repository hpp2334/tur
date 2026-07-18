//! `Tween<T>` — linear interpolation between a beginning and ending value,
//! mirroring Flutter's [`Tween<T>`] abstraction.
//!
//! A `Tween` defines a value range `[begin, end]` and produces intermediate
//! values via [`Tween::lerp`] (and its alias [`Tween::transform`]) given a
//! normalized parameter `t` in `[0, 1]`. Unlike [`crate::Curve`] (which
//! reshapes *time*), a `Tween` interpolates *values*.
//!
//! The generic [`NumTween`] handles numeric types whose default `lerp` is
//! `begin + (end - begin) * t`. Types needing specialized interpolation ship
//! dedicated tweens (e.g. [`ColorTween`], which interpolates RGBA channels
//! component-wise like Flutter's `ColorTween`/`Color.lerp`).
//!
//! [`Tween<T>`]: https://api.flutter.dev/flutter/animation/Tween-class.html

use tur_engine::core::render::Color;

/// A linear interpolation between a beginning and ending value of type `T`.
///
/// Implementors define [`Tween::lerp`]; [`Tween::transform`] has a default
/// impl that clamps `t` to `[0, 1]` then delegates to `lerp` (matching
/// Flutter's `Tween.transform`, which clamps negative time but passes values
/// ≥ 1 through — here we clamp both ends for predictability in layout code).
pub trait Tween<T: Clone>: Clone {
    /// The value at parameter `t` (typically an eased progress in `[0, 1]`).
    fn lerp(&self, t: f64) -> T;

    /// Default-implemented in terms of [`lerp`](Tween::lerp), with `t` clamped
    /// to `[0, 1]`. Override only if a tween needs extrapolation semantics.
    fn transform(&self, t: f64) -> T {
        self.lerp(t.clamp(0.0, 1.0))
    }

    /// Convenience: evaluate at the current value of an `AnimationController`-
    /// style progress. Equivalent to Flutter's `Animatable::evaluate`.
    fn evaluate(&self, t: f64) -> T {
        self.transform(t)
    }
}

/// Generic numeric tween. The default Flutter `Tween<T>` for types satisfying
/// the `+`, `-`, `* double` contract — here specialized to `f64` since that's
/// the universal numeric prop type in tur.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NumTween {
    begin: f64,
    end: f64,
}

impl NumTween {
    pub const fn new(begin: f64, end: f64) -> Self {
        Self { begin, end }
    }
}

impl Tween<f64> for NumTween {
    fn lerp(&self, t: f64) -> f64 {
        self.begin + (self.end - self.begin) * t
    }
}

/// Tween between two [`Color`] values, interpolating RGBA channels
/// component-wise in u8 space (matching Flutter's `Color.lerp` /
/// `ColorTween`). Like Flutter, if either side is unspecified the other side
/// is returned unchanged at the appropriate endpoint; here both `begin` and
/// `end` are non-nullable (tur's `Color` is opaque).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColorTween {
    begin: Color,
    end: Color,
}

impl ColorTween {
    pub const fn new(begin: Color, end: Color) -> Self {
        Self { begin, end }
    }
}

impl Tween<Color> for ColorTween {
    fn lerp(&self, t: f64) -> Color {
        Color::lerp(self.begin, self.end, t)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn num_tween_endpoints_and_midpoint() {
        let tw = NumTween::new(100.0, 200.0);
        assert_eq!(tw.lerp(0.0), 100.0);
        assert_eq!(tw.lerp(1.0), 200.0);
        assert_eq!(tw.lerp(0.5), 150.0);
        assert_eq!(tw.lerp(0.25), 125.0);
    }

    #[test]
    fn num_tween_negative_range() {
        let tw = NumTween::new(200.0, 100.0);
        assert_eq!(tw.lerp(0.0), 200.0);
        assert_eq!(tw.lerp(1.0), 100.0);
        assert_eq!(tw.lerp(0.5), 150.0);
    }

    #[test]
    fn transform_clamps_out_of_range_t() {
        let tw = NumTween::new(0.0, 10.0);
        assert_eq!(tw.transform(-1.0), 0.0);
        assert_eq!(tw.transform(2.0), 10.0);
    }

    #[test]
    fn color_tween_endpoints_exact() {
        let a = Color::rgb(0, 0, 0);
        let b = Color::rgb(255, 255, 255);
        let tw = ColorTween::new(a, b);
        assert_eq!(tw.lerp(0.0), a);
        assert_eq!(tw.lerp(1.0), b);
    }

    #[test]
    fn color_tween_midpoint_rounds() {
        let a = Color::rgb(0, 0, 0);
        let b = Color::rgb(100, 200, 50);
        let mid = ColorTween::new(a, b).lerp(0.5);
        assert_eq!(mid.r(), 50);
        assert_eq!(mid.g(), 100);
        assert_eq!(mid.b(), 25);
    }

    #[test]
    fn color_tween_preserves_alpha_channel() {
        let a = Color::rgba(10, 20, 30, 40);
        let b = Color::rgba(110, 220, 130, 240);
        let mid = ColorTween::new(a, b).lerp(0.5);
        assert_eq!(mid.r(), 60);
        assert_eq!(mid.g(), 120);
        assert_eq!(mid.b(), 80);
        assert_eq!(mid.a(), 140);
    }
}
