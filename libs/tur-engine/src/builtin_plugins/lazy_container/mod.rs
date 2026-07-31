//! Lazy virtualization plugins.
//!
//! Provides:
//! - [`LazyListElement`] — a scrollable, virtualized list that only mounts the
//!   items inside the viewport + overscan, plus its [`LazyListController`] boa
//!   class (`jumpTo`, `onScroll` / `onVisibleRangeChange`).
//! - [`LazyGridElement`] — a scrollable, virtualized grid (row-major tiling,
//!   max cross-axis extent → column count), plus its [`LazyGridController`].
//!
//! Installed into `tur:std` by `TurStdPlugin` via
//! [`install_lazy_container`], which registers the boa classes and returns the
//! JS factory fns to be merged into `std_fns`. From JS's perspective
//! `LazyList`, `LazyGrid`, `createLazyListController` and
//! `createLazyGridController` ship as part of `tur:std`.
//!
//! The scroll-position math (`ScrollPosition`) and the scroll-event payload
//! (`ScrollEvent`) come from the sibling `scroll` plugin.

pub mod lazy_grid;
pub mod lazy_list;

pub use lazy_grid::{
    LazyGridController, LazyGridElement, LazyGridView,
    VisibleRangeChangeEvent as LazyGridVisibleRangeChangeEvent,
};
pub use lazy_list::{LazyListController, LazyListElement, LazyListView, VisibleRangeChangeEvent};

use crate::core::js_runtime::helpers::FnEntry;
use crate::core::plugin::PluginContext;
use crate::error::TurError;

/// Wire the lazy-container plugins into `tur:std`. Called by
/// `TurStdPlugin`'s `register` impl.
///
/// Side effects:
/// - Registers the boa classes [`LazyListController`] and
///   [`LazyGridController`] on `globalThis`.
///
/// Returns: the `LazyList` / `createLazyListController` / `LazyGrid` /
/// `createLazyGridController` factory fns, which the caller merges into
/// `std_fns` before `register_module("tur:std", ...)`.
pub fn install_lazy_container(ctx: &mut PluginContext<'_>) -> Result<Vec<FnEntry>, TurError> {
    ctx.register_class::<LazyListController>()
        .map_err(|e| TurError::Other(format!("failed to register LazyListController: {e}")))?;
    ctx.register_class::<LazyGridController>()
        .map_err(|e| TurError::Other(format!("failed to register LazyGridController: {e}")))?;

    let mut fns = lazy_list::bridge::fns();
    fns.extend(lazy_grid::bridge::fns());
    Ok(fns)
}
