//! Layout primitives — Flutter-inspired flex layout model:
//! - `Column` / `Row` (flex containers, vertical / horizontal main axis).
//! - `Flexible` (flex item; `FlexFit.loose` by default — at most its slot,
//!   smaller allowed) and `Expanded` (= `Flexible` with `FlexFit.tight` —
//!   forced to fill the remaining main-axis space).
//! - `Stack` + `Positioned` (z-axis stacking with anchored children).
//! - `Container` / `SizedBox` (explicit width/height + decoration).
//! - `Grid` (row-major tiling of static children into a max-extent grid).
//! - `Table` (shared-column data table: fixed/flex column widths, reactive
//!   rows via a builder, optional header, stripes + dividers).

pub mod composited_transform;
pub(in crate::builtin_plugins) mod container;
pub mod enums;
pub(in crate::builtin_plugins) mod flex;
pub(in crate::builtin_plugins) mod flex_item;
pub(in crate::builtin_plugins) mod grid;
pub(in crate::builtin_plugins) mod positioned;
pub(in crate::builtin_plugins) mod rut_rows;
pub(in crate::builtin_plugins) mod stack;
pub(in crate::builtin_plugins) mod table;

/// Install the layout families' `tur` host-pkg rows (the kit wraps them):
/// flex / stack / box / sizedbox / positioned / flexible / grid / table.
pub fn install_layout(ctx: &mut crate::core::plugin::PluginRegisterContext) -> Result<(), crate::error::TurError> {
    ctx.push_rut_ext(std::rc::Rc::new(rut_rows::install_decl_ext));
    Ok(())
}

// Temporary: tur-text (still external until Phase E inlines it) consumes
// `ContainerView` for its Input impl. After Phase E moves tur-text into
// `builtin_plugins/text/`, this re-export collapses into a same-plugin
// `pub(in crate::builtin_plugins) use`.
pub use container::ContainerElement;
pub use container::ContainerView;
pub use flex::{FlexElement, FlexView};
pub use flex_item::{FlexibleElement, FlexibleView};
pub use grid::{GridElement, GridView};
pub(crate) use grid::{compute_grid_metrics, cross_offset};
pub use positioned::{PositionedElement, PositionedView};
pub use stack::{StackElement, StackView};
pub use table::{TableColumnDef, TableElement, TableView};
pub use composited_transform::follower::FollowerView;
pub use composited_transform::link::{CompositedLinkState, LayerLink};
pub use composited_transform::target::TargetView;
pub use composited_transform::LayerLinkRegistry;
