//! Layout primitives — Flutter-inspired flex layout model:
//! - `Column` / `Row` (flex containers, vertical / horizontal main axis).
//! - `Expanded` (flex-item factor for filling remaining main-axis space).
//! - `Stack` + `Positioned` (z-axis stacking with anchored children).
//! - `Container` / `SizedBox` (explicit width/height + decoration).
//! - `Grid` (row-major tiling of static children into a max-extent grid).

pub(in crate::builtin_plugins) mod composited_transform;
pub(in crate::builtin_plugins) mod container;
pub mod enums;
pub(in crate::builtin_plugins) mod flex;
pub(in crate::builtin_plugins) mod flex_item;
pub(in crate::builtin_plugins) mod grid;
pub(in crate::builtin_plugins) mod positioned;
pub(in crate::builtin_plugins) mod stack;

// Temporary: tur-text (still external until Phase E inlines it) consumes
// `ContainerView` for its Input impl. After Phase E moves tur-text into
// `builtin_plugins/text/`, this re-export collapses into a same-plugin
// `pub(in crate::builtin_plugins) use`.
pub use container::ContainerElement;
pub use container::ContainerView;
pub use flex::{FlexElement, FlexView};
pub use flex_item::{ExpandedElement, ExpandedView};
pub use grid::{GridElement, GridView};
pub(crate) use grid::{compute_grid_metrics, cross_offset};
pub use positioned::{PositionedElement, PositionedView};
pub use stack::{StackElement, StackView};

use crate::core::js_runtime::helpers::FnEntry;
use crate::core::plugin::PluginContext;
use crate::error::TurError;

/// Install the layout plugin (`Column` / `Row` / `Expanded` / `Stack` /
/// `Positioned` / `Container` / `SizedBox`). Returns the JS factory fns to
/// be merged into `tur:std` by the orchestrator.
pub fn install_layout(_ctx: &mut PluginContext<'_>) -> Result<Vec<FnEntry>, TurError> {
    let mut v: Vec<FnEntry> = Vec::new();
    v.extend(container::bridge::fns());
    v.extend(flex::bridge::fns());
    v.extend(flex_item::bridge::fns());
    v.extend(grid::bridge::fns());
    v.extend(stack::bridge::fns());
    v.extend(positioned::bridge::fns());
    Ok(v)
}
