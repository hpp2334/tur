//! Lazy list virtualization plugin.
//!
//! Provides [`LazyListElement`] (a scrollable, virtualized container that
//! only mounts the items inside the viewport + overscan) and its
//! [`LazyListController`] boa class (drives programmatic `jumpTo` and reports
//! `onScroll` / `onVisibleRangeChange` callbacks).
//!
//! Installed into `builtin:tur/std` by `TurStdPlugin` via
//! [`install_lazy_container`], which registers the boa class and returns the
//! JS factory fns to be merged into `std_fns`. From JS's perspective
//! `LazyList` and `createLazyListController` ship as part of `builtin:tur/std`.
//!
//! The scroll-position math (`ScrollPosition`) and the scroll-event payload
//! (`ScrollEvent`) come from the sibling `scroll` plugin.

pub mod lazy_list;

pub use lazy_list::{LazyListController, LazyListElement, LazyListView, VisibleRangeChangeEvent};

use crate::core::js_runtime::helpers::FnEntry;
use crate::core::plugin::PluginContext;
use crate::error::TurError;

/// Wire the lazy-list plugin into `builtin:tur/std`. Called by
/// `TurStdPlugin`'s `register` impl.
///
/// Side effects:
/// - Registers the boa class [`LazyListController`] on `globalThis`.
///
/// Returns: the `LazyList` / `createLazyListController` factory fns, which
/// the caller merges into `std_fns` before
/// `register_module("builtin:tur/std", ...)`.
pub fn install_lazy_container(
    ctx: &mut PluginContext<'_>,
) -> Result<Vec<FnEntry>, TurError> {
    ctx.register_class::<LazyListController>()
        .map_err(|e| TurError::Other(format!("failed to register LazyListController: {e}")))?;

    Ok(lazy_list::bridge::fns())
}
