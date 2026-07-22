//! Dev tooling — JS-exposed debug helpers.
//!
//! `__tur._dev_tool_element_tree` / `__tur._dev_tool_get_element` and the
//! public `turDevTool` global that wraps them. Used by the playground to
//! introspect element-tree state from JS.

pub mod dev_tool;
