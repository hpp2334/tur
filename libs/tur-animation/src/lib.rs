//! Animation subsystem for tur.
//!
//! Provides [`TurAnimationPlugin`] — a plugin that registers:
//!   - the `AnimationController` boa class,
//!   - the `AnimationSubsystem` flush participant (ticks active controllers
//!     once per frame via the engine's [`Subsystem`](tur_engine::core::subsystem::Subsystem)
//!     mechanism),
//!   - the `tur:animation` JS module combining the native bridge fn
//!     (`createAnimationController`) with the JS-defined implicit-animation
//!     widgets (`AnimatedContainer`, `AnimatedOpacity`, `AnimatedPositioned`,
//!     `Tween`, `ColorTween`). The visual effects `Opacity` / `Transform`
//!     used by those widgets ship as part of `tur:std`.
//!
//! `TurAnimationPlugin` carries no per-instance state; the animation manager
//! lives inside the `AnimationSubsystem` for the lifetime of the app.

pub mod controller;
pub mod curve;
pub mod event;
pub mod flush_hook;
pub mod manager;
pub mod plugin;
pub mod tween;

pub use controller::AnimationController;
pub use curve::Curve;
pub use flush_hook::AnimationSubsystem;
pub use manager::AnimationManager;
pub use plugin::TurAnimationPlugin;
pub use tween::{ColorTween, NumTween, Tween};
