//! Builtin plugins — feature bundles bundled with the engine itself.
//!
//! Each plugin sub-folder exports a single `pub fn install_<name>(ctx:
//! &mut PluginRegisterContext) -> Result<(), TurError>` that registers its
//! subsystems + plugin state + rut-ext pushes. Elements materialize
//! pure-Rust views (authored through the rut rows in `core::rut_runtime`);
//! there is no JS bridge rail. Sub-modules are `pub(in
//! crate::builtin_plugins)` so sibling plugins can share internals (e.g.
//! text uses scroll's `dispatch_wheel`), but `core/` and external crates
//! cannot reach past `install_xxx`.
//!
//! `core/` keeps only pure infrastructure (trait defs, app loop, event
//! queues, render/layout/view primitives, contract types).
//!
//! [`TurStdPlugin`] in `std.rs` is the orchestrator that calls every
//! plugin's `install_xxx`.

pub mod clipboard;
pub mod control_flow;
pub mod effects;
pub mod focus;
pub mod gesture;
pub mod image;
pub mod input;
pub mod layout;
pub mod lazy_container;
pub mod lifecycle;
pub mod scroll;
pub mod std;
pub mod text;
pub mod virtual_app;

pub use clipboard::TurClipboardPlugin;
pub use std::TurStdPlugin;
