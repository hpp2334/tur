//! Per-property state for an implicit animation. Each animated element holds
//! one [`AnimatedProp<T>`] per animatable property and resolves it against
//! the shared [`super::driver::ImplicitDriver`]'s eased `t`.
//!
//! Mirrors Flutter's `_AnimatedEvaluation<T>` + per-property `Tween` inside
//! `ImplicitlyAnimatedWidget`: each prop keeps its own `begin`/`end` and
//! interpolates independently, all sharing one timeline.

/// State for a single animatable property of type `T`.
///
/// - `begin` / `end` define the current tween range.
/// - On mount, `begin == end == target` (seeded), so the first paint shows
///   the target with no animation — matches Flutter's first-frame rule.
/// - On a target change, the element captures the *currently displayed*
///   value (the lerp result at the previous eased `t`) as the new `begin`,
///   sets `end = new_target`, and retargets the driver. This keeps the
///   displayed value continuous across retargets (no jump).
#[derive(Clone, Debug)]
pub struct AnimatedProp<T: Clone + PartialEq> {
    begin: Option<T>,
    end: Option<T>,
    seeded: bool,
}

impl<T: Clone + PartialEq> AnimatedProp<T> {
    pub fn new() -> Self {
        AnimatedProp {
            begin: None,
            end: None,
            seeded: false,
        }
    }

    /// Construct a prop already seeded at `target` (`begin == end == target`,
    /// no animation). Used at `View::build` so the first layout paints the
    /// target without waiting for an Effect pass (which doesn't run on mount,
    /// since no atom is dirty yet).
    pub fn seeded(target: Option<T>) -> Self {
        AnimatedProp {
            begin: target.clone(),
            end: target,
            seeded: true,
        }
    }

    /// The current target value (the `end` of the tween), if any.
    pub fn target(&self) -> Option<&T> {
        self.end.as_ref()
    }

    /// Feed the latest resolved target for this prop. Returns
    /// `(target_changed, is_first_seed)` so the caller can decide whether to
    /// retarget the driver.
    ///
    /// - First ever target: seeds `begin = end = target`, no animation.
    /// - Subsequent target that differs: leaves `begin` for the caller to
    ///   rebase to the current displayed value via [`rebase_begin`], then
    ///   sets `end = target`.
    /// - Target unchanged: no-op.
    pub fn update_target(&mut self, new_target: Option<T>) -> (bool, bool) {
        if !self.seeded {
            self.begin = new_target.clone();
            self.end = new_target;
            self.seeded = true;
            return (false, true);
        }
        if self.end == new_target {
            return (false, false);
        }
        self.end = new_target;
        (true, false)
    }

    /// On a retarget, capture the currently-displayed value (the element
    /// computes it as `lerp(begin, end, prev_eased_t)`) as the new `begin`.
    /// This makes the timeline restart from the visible value, preserving
    /// continuity. Only meaningful after [`update_target`] reports a change.
    pub fn rebase_begin(&mut self, displayed: Option<T>) {
        self.begin = displayed;
    }

    /// Evaluate the tween at eased parameter `t` using the supplied lerp
    /// closure. Returns `None` if no target was ever set.
    pub fn evaluate<F: FnOnce(&T, &T, f64) -> T>(&self, t: f64, lerp: F) -> Option<T> {
        match (self.begin.as_ref(), self.end.as_ref()) {
            (Some(b), Some(e)) => Some(lerp(b, e, t)),
            (None, Some(e)) => Some(e.clone()),
            _ => self.end.clone(),
        }
    }
}

impl<T: Clone + PartialEq> Default for AnimatedProp<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_target_seeds_without_animation() {
        let mut p: AnimatedProp<f64> = AnimatedProp::new();
        let (changed, first) = p.update_target(Some(10.0));
        assert!(!changed);
        assert!(first);
        // begin == end so lerp at any t yields 10.0
        assert_eq!(p.evaluate(0.0, |b, e, t| b + (e - b) * t), Some(10.0));
        assert_eq!(p.evaluate(0.5, |b, e, t| b + (e - b) * t), Some(10.0));
        assert_eq!(p.evaluate(1.0, |b, e, t| b + (e - b) * t), Some(10.0));
    }

    #[test]
    fn unchanged_target_is_noop() {
        let mut p: AnimatedProp<f64> = AnimatedProp::new();
        p.update_target(Some(10.0));
        let (changed, first) = p.update_target(Some(10.0));
        assert!(!changed);
        assert!(!first);
    }

    #[test]
    fn changed_target_reports_change() {
        let mut p: AnimatedProp<f64> = AnimatedProp::new();
        p.update_target(Some(10.0));
        let (changed, first) = p.update_target(Some(20.0));
        assert!(changed);
        assert!(!first);
    }

    #[test]
    fn rebase_then_evaluate_uses_displayed_as_begin() {
        let mut p: AnimatedProp<f64> = AnimatedProp::new();
        p.update_target(Some(0.0));
        // Animate toward 100. Mid-flight at eased_t=0.25, displayed = 25.
        p.update_target(Some(100.0));
        let displayed = p.evaluate(0.25, |b, e, t| b + (e - b) * t);
        assert_eq!(displayed, Some(25.0));
        // Retarget to 200: first set the new end, then rebase begin to the
        // currently-displayed value so the timeline restarts from 25.
        p.update_target(Some(200.0));
        p.rebase_begin(displayed);
        // After retarget, t=0 → begin (25), t=1 → end (200).
        assert_eq!(p.evaluate(0.0, |b, e, t| b + (e - b) * t), Some(25.0));
        assert_eq!(p.evaluate(1.0, |b, e, t| b + (e - b) * t), Some(200.0));
        assert_eq!(p.evaluate(0.5, |b, e, t| b + (e - b) * t), Some(112.5));
    }

    #[test]
    fn none_target_yields_none() {
        let mut p: AnimatedProp<f64> = AnimatedProp::new();
        assert_eq!(p.update_target(None), (false, true));
        assert_eq!(p.evaluate(0.5, |b, e, t| b + (e - b) * t), None);
    }
}
