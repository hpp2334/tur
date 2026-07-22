//! Control-flow fragments: conditional + iterative + grouping primitives.
//!
//! - `Condition` / `Switch` / `Each` — fragment-based control flow.
//! - `Fragment` — grouping primitive (no layout footprint).

pub(in crate::builtin_plugins) mod condition;
pub(in crate::builtin_plugins) mod each;
pub(in crate::builtin_plugins) mod fragment;
pub(in crate::builtin_plugins) mod switch;

use crate::core::js_runtime::helpers::FnEntry;
use crate::core::plugin::PluginContext;
use crate::error::TurError;

/// Install the control-flow plugin (`Condition` / `Switch` / `Each` /
/// `Fragment`). Returns the JS factory fns to be merged into
/// `tur:std` by the orchestrator (`TurStdPlugin`).
pub fn install_control_flow(
    _ctx: &mut PluginContext<'_>,
) -> Result<Vec<FnEntry>, TurError> {
    let mut v: Vec<FnEntry> = Vec::new();
    v.extend(condition::bridge::fns());
    v.extend(switch::bridge::fns());
    v.extend(each::bridge::fns());
    v.extend(fragment::bridge::fns());
    Ok(v)
}
