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
