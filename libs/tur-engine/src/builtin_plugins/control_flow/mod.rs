//! Control-flow fragments: conditional + iterative + grouping primitives.
//!
//! - `Condition` / `Switch` / `Each` — fragment-based control flow.
//! - `Fragment` — grouping primitive (no layout footprint).

pub(in crate::builtin_plugins) mod condition;
pub(in crate::builtin_plugins) mod each;
pub mod fragment;
pub(in crate::builtin_plugins) mod rut_rows;
pub(in crate::builtin_plugins) mod switch;

// The rut rail authors every view here (through this plugin's rows).
pub use condition::ConditionView;
pub use each::{EachBuilder, EachView};
pub use fragment::FragmentView;
pub use switch::{Prebuilt, SwitchKey, SwitchView};

/// Install the control-flow families' `tur` host-pkg rows (the kit wraps
/// them): condition / switch / each / fragment.
pub fn install_control_flow(ctx: &mut crate::core::plugin::PluginRegisterContext) -> Result<(), crate::error::TurError> {
    ctx.push_rut_ext(std::rc::Rc::new(rut_rows::install_ext));
    Ok(())
}
