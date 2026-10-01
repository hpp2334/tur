//! Lifecycle plugin — `LifecycleView` wraps a pre-built child with
//! mount/unmount intent callbacks (the C7 rut rows' `el_lifecycle`).

pub(in crate::builtin_plugins) mod element;
pub(in crate::builtin_plugins) mod layout;
pub(in crate::builtin_plugins) mod render;

pub(crate) use element::{LifecycleFactory, LifecycleView};
