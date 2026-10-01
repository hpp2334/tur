//! Control-flow fragments: conditional + iterative + grouping primitives.
//!
//! - `Condition` / `Switch` / `Each` — fragment-based control flow.
//! - `Fragment` — grouping primitive (no layout footprint).

pub(in crate::builtin_plugins) mod condition;
pub(in crate::builtin_plugins) mod each;
pub mod fragment;
pub(in crate::builtin_plugins) mod switch;

// The rut rail (`core::rut_runtime`) authors every view here.
pub use condition::ConditionView;
pub use each::{EachBuilder, EachView};
pub use fragment::FragmentView;
pub use switch::{Prebuilt, SwitchKey, SwitchView};
