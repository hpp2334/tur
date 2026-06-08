use crate::color::Color;
use crate::layout::{Offset, Size};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationCurve {
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
}

impl AnimationCurve {
    pub fn apply(&self, t: f64) -> f64 {
        match self {
            AnimationCurve::Linear => t,
            AnimationCurve::EaseIn => t * t,
            AnimationCurve::EaseOut => t * (2.0 - t),
            AnimationCurve::EaseInOut => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    -1.0 + (4.0 - 2.0 * t) * t
                }
            }
        }
    }
}

impl std::str::FromStr for AnimationCurve {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "easeIn" => AnimationCurve::EaseIn,
            "easeOut" => AnimationCurve::EaseOut,
            "easeInOut" => AnimationCurve::EaseInOut,
            _ => AnimationCurve::Linear,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Tween {
    Float { begin: f64, end: f64 },
    Color { begin: Color, end: Color },
    Size { begin: Size, end: Size },
    Offset { begin: Offset, end: Offset },
}

impl Tween {
    pub fn lerp(&self, t: f64) -> AnimatableValue {
        match self {
            Tween::Float { begin, end } => {
                AnimatableValue::Float(begin + (end - begin) * t)
            }
            Tween::Color { begin, end } => {
                AnimatableValue::Color(Color::rgba(
                    lerp_u8(begin.r(), end.r(), t),
                    lerp_u8(begin.g(), end.g(), t),
                    lerp_u8(begin.b(), end.b(), t),
                    lerp_u8(begin.a(), end.a(), t),
                ))
            }
            Tween::Size { begin, end } => AnimatableValue::Size(Size::new(
                begin.width + (end.width - begin.width) * t,
                begin.height + (end.height - begin.height) * t,
            )),
            Tween::Offset { begin, end } => AnimatableValue::Offset(Offset::new(
                begin.x + (end.x - begin.x) * t,
                begin.y + (end.y - begin.y) * t,
            )),
        }
    }

    pub fn evaluate(&self, t: f64, curve: &AnimationCurve) -> AnimatableValue {
        self.lerp(curve.apply(t))
    }
}

fn lerp_u8(a: u8, b: u8, t: f64) -> u8 {
    (a as f64 + (b as f64 - a as f64) * t).round() as u8
}

#[derive(Debug, Clone, PartialEq)]
pub enum AnimatableValue {
    Float(f64),
    Color(Color),
    Size(Size),
    Offset(Offset),
}

impl AnimatableValue {
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            AnimatableValue::Float(v) => Some(*v),
            _ => None,
        }
    }

    pub fn as_color(&self) -> Option<&Color> {
        match self {
            AnimatableValue::Color(c) => Some(c),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TransitionConfig {
    pub duration_ms: u64,
    pub curve: AnimationCurve,
}
