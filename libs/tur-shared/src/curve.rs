/// A time-remapping easing function `f64 -> f64`, mirroring Flutter's
/// `Curve` (an `Animatable<double>` whose `transform(t)` reshapes linear
/// time). Named `Curve` to match Flutter terminology exactly; the previous
/// `AnimationCurve` name was a misnomer conflating this with `Tween<T>`.
///
/// Compare with [`crate::tween::Tween`], which interpolates *values* across a
/// `begin..end` range using a parameter `t` (typically produced by applying a
/// `Curve` to an `AnimationController`'s raw progress).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Curve {
    #[default]
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
}

impl Curve {
    /// Apply this curve to a linear progress value `t` in `[0, 1]`,
    /// returning the eased progress. Values outside `[0, 1]` are passed
    /// through unchanged (callers clamp where appropriate).
    pub fn transform(&self, t: f64) -> f64 {
        match self {
            Curve::Linear => t,
            Curve::EaseIn => t * t,
            Curve::EaseOut => t * (2.0 - t),
            Curve::EaseInOut => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    -1.0 + (4.0 - 2.0 * t) * t
                }
            }
        }
    }
}

impl std::str::FromStr for Curve {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "easeIn" => Curve::EaseIn,
            "easeOut" => Curve::EaseOut,
            "easeInOut" => Curve::EaseInOut,
            _ => Curve::Linear,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_is_identity() {
        assert_eq!(Curve::Linear.transform(0.0), 0.0);
        assert_eq!(Curve::Linear.transform(0.5), 0.5);
        assert_eq!(Curve::Linear.transform(1.0), 1.0);
    }

    #[test]
    fn ease_endpoints() {
        for c in [Curve::EaseIn, Curve::EaseOut, Curve::EaseInOut] {
            assert!((c.transform(0.0)).abs() < 1e-9, "0 endpoint for {c:?}");
            assert!((c.transform(1.0) - 1.0).abs() < 1e-9, "1 endpoint for {c:?}");
        }
    }

    #[test]
    fn ease_in_out_midpoint_matches_in_out_formula() {
        // EaseInOut at 0.25 = 2 * 0.25^2 = 0.125 (first branch, t < 0.5)
        assert!((Curve::EaseInOut.transform(0.25) - 0.125).abs() < 1e-9);
        // EaseInOut at 0.75 = -1 + (4 - 1.5) * 0.75 = 0.875 (second branch)
        assert!((Curve::EaseInOut.transform(0.75) - 0.875).abs() < 1e-9);
    }

    #[test]
    fn from_str_round_trip() {
        assert_eq!("linear".parse::<Curve>().unwrap(), Curve::Linear);
        assert_eq!("easeIn".parse::<Curve>().unwrap(), Curve::EaseIn);
        assert_eq!("easeOut".parse::<Curve>().unwrap(), Curve::EaseOut);
        assert_eq!("easeInOut".parse::<Curve>().unwrap(), Curve::EaseInOut);
        assert_eq!("unknown".parse::<Curve>().unwrap(), Curve::Linear);
    }
}
