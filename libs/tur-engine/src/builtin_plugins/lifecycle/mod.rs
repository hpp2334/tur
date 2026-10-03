//! Lifecycle plugin — `LifecycleView` wraps a pre-built child with
//! mount/unmount intent callbacks (the C7 rut rows' `el_lifecycle`).

pub(in crate::builtin_plugins) mod element;
pub(in crate::builtin_plugins) mod layout;
pub(in crate::builtin_plugins) mod render;
pub(in crate::builtin_plugins) mod rut_rows;

/// Install the lifecycle family's `tur` host-pkg rows (the kit wraps
/// them): mount/destroy intents around a pre-built child.
pub fn install_lifecycle(ctx: &mut crate::core::plugin::PluginRegisterContext) -> Result<(), crate::error::TurError> {
    ctx.push_rut_ext(std::rc::Rc::new(rut_rows::install_ext));
    Ok(())
}

pub(crate) use element::{LifecycleFactory, LifecycleView};
