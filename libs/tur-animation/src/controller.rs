use std::cell::RefCell;
use std::rc::Rc;

use crate::Curve;
use tur_engine::core::edgy::mutation::{MutationHandle, PendingMutationInvocationQueue};

use crate::event::{AnimationEndEvent, AnimationTickEvent};
use crate::manager::AnimationManager;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationStatus {
    Stopped,
    Forward,
    Reverse,
    Completed,
    Paused,
}

/// Repeat policy for an [`AnimationController`]. Mirrors the user-facing
/// JS API: `repeat(count)` accepts a positive integer or the string
/// `"infinite"`. Internally we model both as a single enum so the tick
/// math has a single point that gates completion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepeatMode {
    /// Play exactly `n` iterations, then transition to `Completed`.
    Finite(u64),
    /// Loop forever. `tick_compute` never sets status to `Completed`;
    /// `onEnd` never fires. The value cycles through `[0, 1]` via the
    /// existing `rem_euclid(1.0)` wrap math.
    Infinite,
}

impl Default for RepeatMode {
    fn default() -> Self {
        RepeatMode::Finite(1)
    }
}

/// One explicit animation transport: value interpolation driven by the
/// engine clock, registered into the shared [`AnimationManager`] while
/// playing. Pure Rust state — the `tur_host` pkg rows mint and drive it.
pub struct AnimationController {
    duration_ms: u64,
    curve: Curve,
    value: f64,
    status: AnimationStatus,
    repeat_mode: RepeatMode,
    current_iteration: u64,
    /// The `value` captured when this animation segment started (set by
    /// `forward`/`reverse`/`resume`/`seek`). Used by `tick` to compute the
    /// current value relative to the start, which makes `speed`, `seek`,
    /// and `pause`/`resume` work correctly.
    value_at_start: f64,
    /// Time multiplier (default 1.0). Mutated by `set_speed`. Higher values
    /// play faster; 0.5 = half speed, 2.0 = double speed.
    speed: f64,
    /// The direction the controller was traveling before `pause()` — used
    /// by `resume()` to pick up where it left off.
    paused_direction: Option<AnimationStatus>,
    /// Atom-backed callback handle for `onTick`. Resolved via the reactive
    /// store at flush time (just like every other event handler), so the
    /// callback runs after all `RefMut` borrows are released.
    on_tick: Option<MutationHandle<AnimationTickEvent>>,
    /// Atom-backed callback handle for `onEnd`. Same dispatch path as
    /// `on_tick`.
    on_end: Option<MutationHandle<AnimationEndEvent>>,
    start_time_ms: Option<u64>,
    animation_manager: Option<Rc<RefCell<AnimationManager>>>,
    /// The engine-wide mutation queue. Set by `tur_create_animation_controller`
    /// at construction time. Used to defer `onTick` / `onEnd` invocations to
    /// the next flush — never invoke these callbacks synchronously while
    /// holding a `RefMut` on the controller.
    mutation_queue: Option<Rc<RefCell<PendingMutationInvocationQueue>>>,
}

impl AnimationController {
    pub fn new(duration_ms: u64, curve: Curve) -> Self {
        Self {
            duration_ms,
            curve,
            value: 0.0,
            status: AnimationStatus::Stopped,
            repeat_mode: RepeatMode::default(),
            current_iteration: 0,
            value_at_start: 0.0,
            speed: 1.0,
            paused_direction: None,
            on_tick: None,
            on_end: None,
            start_time_ms: None,
            animation_manager: None,
            mutation_queue: None,
        }
    }

    pub fn is_active(&self) -> bool {
        matches!(
            self.status,
            AnimationStatus::Forward | AnimationStatus::Reverse
        )
    }

    // ---- inherent control surface (the class methods + the rut rows) ----
    //
    // State transitions live here; the class methods add the clock read +
    // the manager re-registration around them (the borrow must not nest).

    /// Start playing forward from 0 at `now_ms`.
    pub fn forward_at(&mut self, now_ms: u64) {
        self.status = AnimationStatus::Forward;
        self.start_time_ms = Some(now_ms);
        self.current_iteration = 0;
        self.value = 0.0;
        self.value_at_start = 0.0;
        self.paused_direction = None;
        self.enqueue_tick(0.0);
    }

    /// Start playing in reverse from 1 at `now_ms`.
    pub fn reverse_at(&mut self, now_ms: u64) {
        self.status = AnimationStatus::Reverse;
        self.start_time_ms = Some(now_ms);
        self.current_iteration = 0;
        self.value = 1.0;
        self.value_at_start = 1.0;
        self.paused_direction = None;
        self.enqueue_tick(1.0);
    }

    /// Stop (value freezes; status `Stopped`).
    pub fn stop_now(&mut self) {
        self.status = AnimationStatus::Stopped;
        self.start_time_ms = None;
        self.paused_direction = None;
    }

    /// Pause at the current value (ticks once at `now_ms` first so `value`
    /// reflects the pause moment).
    pub fn pause_at(&mut self, now_ms: u64) {
        if self.is_active() {
            let _ = self.tick_compute(now_ms);
            self.paused_direction = Some(self.status);
            self.status = AnimationStatus::Paused;
            self.start_time_ms = None;
        }
    }

    /// Resume the paused direction from the current value at `now_ms`.
    pub fn resume_at(&mut self, now_ms: u64) {
        if self.status != AnimationStatus::Paused {
            return;
        }
        let direction = self.paused_direction.unwrap_or(AnimationStatus::Forward);
        self.status = direction;
        self.value_at_start = self.value;
        self.start_time_ms = Some(now_ms);
        self.paused_direction = None;
    }

    /// Jump to `t` (clamped 0..1), re-basing the timeline when active.
    pub fn seek_to(&mut self, t: f64, now_ms: u64) {
        let t = t.clamp(0.0, 1.0);
        self.value = t;
        self.value_at_start = t;
        if self.is_active() {
            self.rebase_start_to_current_value(now_ms);
        }
        self.enqueue_tick(t);
    }

    /// Set the time multiplier, re-basing the timeline when active.
    pub fn set_speed_to(&mut self, s: f64, now_ms: u64) {
        if self.is_active() {
            let _ = self.tick_compute(now_ms);
            self.speed = s;
            self.value_at_start = self.value;
            self.start_time_ms = Some(now_ms);
        } else {
            self.speed = s;
        }
    }

    /// Set the repeat mode (resets the iteration counter).
    pub fn set_repeat_mode(&mut self, mode: RepeatMode) {
        self.repeat_mode = mode;
        self.current_iteration = 0;
    }

    /// Wire the `onTick` callback (the rut rail's intent mutation).
    pub fn set_on_tick(&mut self, m: MutationHandle<AnimationTickEvent>) {
        self.on_tick = Some(m);
    }

    /// Wire the `onEnd` callback (the rut rail's intent mutation).
    pub fn set_on_end(&mut self, m: MutationHandle<AnimationEndEvent>) {
        self.on_end = Some(m);
    }

    /// The current eased value (`0.0` before the first tick).
    pub fn value(&self) -> f64 {
        self.value
    }

    /// The status name (the JS getter's vocabulary).
    pub fn status_name(&self) -> &'static str {
        match self.status {
            AnimationStatus::Stopped => "stopped",
            AnimationStatus::Forward => "forward",
            AnimationStatus::Reverse => "reverse",
            AnimationStatus::Completed => "completed",
            AnimationStatus::Paused => "paused",
        }
    }

    pub fn set_animation_manager(&mut self, mgr: Rc<RefCell<AnimationManager>>) {
        self.animation_manager = Some(mgr);
    }

    /// The manager handle, for control surfaces that re-register after a
    /// state change (the rut rows; the borrow must not span the register).
    pub fn manager_handle(&self) -> Option<Rc<RefCell<AnimationManager>>> {
        self.animation_manager.clone()
    }

    /// Set the engine-wide mutation queue. Called once at construction by
    /// `tur_create_animation_controller`. Required for `onTick` / `onEnd`
    /// dispatch — if `None`, callbacks are silently dropped.
    pub fn set_mutation_queue(&mut self, queue: Rc<RefCell<PendingMutationInvocationQueue>>) {
        self.mutation_queue = Some(queue);
    }

    /// Enqueue an `onTick(eased_t)` invocation on the mutation queue. The
    /// callback fires during the next `flush_pending_mutations` pass, after
    /// any active `RefMut` borrow on this controller (or any other) is
    /// released. Safe to call while holding a `RefMut` — this only clones
    /// a `Mutation` handle and pushes a `Box<dyn IntoJsArgs>` onto a separate `RefCell`.
    fn enqueue_tick(&self, eased_t: f64) {
        if let (Some(queue), Some(m)) = (&self.mutation_queue, self.on_tick) {
            queue.borrow_mut().push(m, AnimationTickEvent(eased_t));
        }
    }

    /// Enqueue an `onEnd()` invocation. See `enqueue_tick` for the dispatch
    /// rationale.
    fn enqueue_end(&self) {
        if let (Some(queue), Some(m)) = (&self.mutation_queue, self.on_end) {
            queue.borrow_mut().push(m, AnimationEndEvent);
        }
    }

    /// Recompute `start_time_ms` so the next `tick` continues from the
    /// current `value`. Used after `seek` and `set_speed` to keep the math
    /// consistent.
    fn rebase_start_to_current_value(&mut self, now_ms: u64) {
        if self.duration_ms == 0 {
            self.start_time_ms = Some(now_ms);
            return;
        }
        // We have value = value_at_start + direction * (elapsed * speed / duration).
        // Solve for elapsed: elapsed = (value - value_at_start) * duration / (direction * speed).
        let direction: f64 = if self.status == AnimationStatus::Forward {
            1.0
        } else {
            -1.0
        };
        let delta = self.value - self.value_at_start;
        let elapsed_ms = if (direction * self.speed).abs() < 1e-9 {
            0.0
        } else {
            (delta * self.duration_ms as f64) / (direction * self.speed)
        };
        // elapsed_ms may be negative if value moved backwards — clamp to >=0
        // to keep start_time in the past.
        let elapsed_ms = elapsed_ms.max(0.0);
        self.start_time_ms = Some(now_ms.saturating_sub(elapsed_ms as u64));
    }

    /// Compute one tick of the animation: update `value` and `status` based
    /// on the elapsed time, and **enqueue** (not fire) the `onTick` / `onEnd`
    /// callbacks on the mutation queue. The callbacks fire during the next
    /// `flush_pending_mutations` pass, after all `RefMut` borrows are
    /// released — a callback that re-enters the controller cannot hit a live
    /// borrow.
    ///
    /// Returns `true` if the controller ticked (active and had a start time),
    /// `false` if it was idle.
    pub fn tick_compute(&mut self, now_ms: u64) -> bool {
        if !self.is_active() {
            return false;
        }
        let Some(start) = self.start_time_ms else {
            return false;
        };

        let direction: f64 = if self.status == AnimationStatus::Forward {
            1.0
        } else {
            -1.0
        };

        let elapsed_ms = now_ms.saturating_sub(start) as f64;
        let scaled_elapsed_ms = elapsed_ms * self.speed;
        let progress_delta = scaled_elapsed_ms / self.duration_ms.max(1) as f64;

        let new_value = self.value_at_start + direction * progress_delta;

        let (max_iterations, infinite) = match self.repeat_mode {
            RepeatMode::Finite(n) => (n, false),
            RepeatMode::Infinite => (u64::MAX, true),
        };
        let completed = !infinite
            && if direction > 0.0 {
                new_value >= max_iterations as f64
            } else {
                // Reverse starts at value_at_start (typically 1.0) and decreases.
                new_value <= (1.0 - max_iterations as f64)
            };

        let t = if completed {
            if direction > 0.0 { 1.0 } else { 0.0 }
        } else if infinite || max_iterations > 1 {
            let frac = new_value.rem_euclid(1.0);
            if direction > 0.0 { frac } else { 1.0 - frac }
        } else {
            new_value.clamp(0.0, 1.0)
        };

        self.value = t;
        self.current_iteration = if completed {
            max_iterations
        } else if infinite {
            // For infinite mode, current_iteration grows unboundedly; cap
            // at u64::MAX to avoid overflow. Useful only for diagnostics —
            // the user reads `value`, not `current_iteration`.
            (new_value.max(0.0).floor() as u64).saturating_add(0)
        } else if max_iterations > 1 {
            (new_value.max(0.0).floor() as u64).min(max_iterations)
        } else {
            0
        };

        let eased_t = self.curve.transform(t);

        if completed {
            self.status = AnimationStatus::Completed;
            self.value = if direction > 0.0 { 1.0 } else { 0.0 };
            self.paused_direction = None;
        }

        // Enqueue callbacks — they fire later, outside the RefMut borrow.
        self.enqueue_tick(eased_t);
        if completed {
            self.enqueue_end();
        }
        true
    }
}
