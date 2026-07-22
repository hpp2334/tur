//! Lifecycle plugin — `lifecycleView(factory)` for wrapping a JS factory
//! `() => { element, onMounted$?, beforeDestroy$? }` with mount/unmount
//! callbacks.

pub(in crate::builtin_plugins) mod bridge;
pub(in crate::builtin_plugins) mod element;
pub(in crate::builtin_plugins) mod layout;
pub(in crate::builtin_plugins) mod render;

pub(in crate::builtin_plugins) use element::LifecycleView;

use crate::core::js_runtime::helpers::FnEntry;
use crate::core::plugin::PluginContext;
use crate::error::TurError;

/// Install the lifecycle plugin (`lifecycleView`). Returns the JS factory
/// fns to be merged into `builtin:tur/std` by the orchestrator.
pub fn install_lifecycle(
    _ctx: &mut PluginContext<'_>,
) -> Result<Vec<FnEntry>, TurError> {
    Ok(bridge::fns())
}
