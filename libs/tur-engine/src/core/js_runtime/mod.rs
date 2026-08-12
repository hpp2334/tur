//! Boa engine runtime plumbing — low-level JS integration substrate.
//!
//! Everything in this folder is generic JS-runtime infrastructure shared by
//! every bridge fn across the engine and every external crate that writes
//! element bridges (`tur-text`, `tur-image`, `tur-scroll`, …). It owns no
//! `tur:*` module exports — engine-level JS fns/consts live in their
//! domain folders (`core::render::brush`, `core::app::render`, `core::async_::task`,
//! `core::dev`, `domains::layout::enums`).
//!
//! - [`module_loader`] — `TurModuleLoader` + synthetic-module builders
//!   (`build_native_module` / `build_fn_module` / `bound_native`).
//! - [`opaque`] — `BoaOpaque<T>` generic `NativeObject` downcast helper.
//! - [`helpers`] — bridge plumbing: `FnEntry` / `ConstEntry` / `Ptr` type
//!   aliases, `extract_js_ctx`, `require_props_object`, `wrap_view`,
//!   `TurNodeHandle`.
//! - [`instance_context`] — `TurInstanceContext` (the boa `JsData` handle
//!   passed as `args[0]` to every ctx-first bridge fn).
//! - [`js_props`] — `JsProps` (the prop-reader every element's `from_js` uses).
//! - [`js_value`] — `FromJs` / `IntoJs` unified value-conversion traits.

pub use crate::core::js_runtime::helpers::{ConstEntry, FnEntry};

pub mod helpers;
pub mod instance_context;
pub mod js_props;
pub mod js_value;
pub mod module_loader;
pub mod opaque;

pub use helpers::{TurNodeHandle, extract_js_ctx};
pub use instance_context::TurInstanceContext;
pub use js_props::JsProps;
pub use module_loader::TurModuleLoader;
pub use opaque::BoaOpaque;
