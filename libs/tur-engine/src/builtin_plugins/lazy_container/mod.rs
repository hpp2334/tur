//! Lazy virtualization plugins.
//!
//! Provides:
//! - [`LazyListElement`] — a scrollable, virtualized list that only mounts the
//!   items inside the viewport + overscan, plus its plain-Rust
//!   [`LazyListController`] (`jump_to`, `onScroll` / `onVisibleRangeChange`).
//! - [`LazyGridElement`] — a scrollable, virtualized grid (row-major tiling,
//!   max cross-axis extent → column count), plus its [`LazyGridController`].
//!
//! Elements materialize pure-Rust views (authored through the
//! `core::rut_runtime` rows); controllers are plain Rust types — bound by
//! whoever holds them, firing their callbacks through the mutation queue.
//!
//! The scroll-position math (`ScrollPosition`) and the scroll-event payload
//! (`ScrollEvent`) come from the sibling `scroll` plugin.

pub mod item_builder;
pub mod lazy_grid;
pub mod lazy_list;

pub use lazy_grid::{
    LazyGridController, LazyGridElement, LazyGridView,
    VisibleRangeChangeEvent as LazyGridVisibleRangeChangeEvent,
};
pub use lazy_list::{LazyListController, LazyListElement, LazyListView, VisibleRangeChangeEvent};
