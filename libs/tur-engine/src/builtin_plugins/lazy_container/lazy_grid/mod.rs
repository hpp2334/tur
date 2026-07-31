//! `LazyGrid` — a scrollable, virtualized grid.
//!
//! Mirrors [`LazyList`](crate::builtin_plugins::lazy_container::lazy_list): a
//! scroll container that lays built cells into a row-major grid, clips to the
//! viewport, and only mounts the cells inside the viewport + overscan. Remount
//! happens inside `perform_layout` (via `LayoutViewCx`) using the real viewport.
//!
//! Cell sizing is uniform (max cross-axis extent → column count, then either
//! fixed `mainAxisExtent` or `childAspectRatio`), so positioning is analytic
//! — no per-cell extent cache or positioning anchor is needed (unlike
//! LazyList's variable-height items).
//!
//! Scroll math (`ScrollPosition`) + event payload (`ScrollEvent`) come from the
//! sibling `scroll` plugin.

pub mod bridge;
pub mod controller;
mod element;
mod layout;
mod render;

pub use controller::LazyGridController;
pub use element::{LazyGridElement, LazyGridView, VisibleRangeChangeEvent};
