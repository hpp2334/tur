//! Virtual apps, plugin half — `VirtualAppView` (the element that hosts a
//! complete nested engine instance), `createModuleSource` /
//! `createVirtualAppController` (the JS bridge), and the
//! `VirtualAppSubsystem` (status/frame consumption + layout-driven resize).
//!
//! The engine seam (host-side hosting, frame forwarding, the child shell)
//! lives in [`crate::core::virtual_app`]. Installed into `tur:std` by
//! `TurStdPlugin` via [`install_virtual_app`] — the same
//! `install_xxx(ctx) -> …` pattern as `install_text` / `install_image`.
//!
//! JS surface (all on `tur:std`):
//!
//! ```js
//! import { createModuleSource, createVirtualAppController, VirtualAppView } from "tur:std";
//!
//! const src = createModuleSource(compiledJs);     // opaque handle, never the string
//! const app = createVirtualAppController({ source: src, pool: "virtual", keepAlive: false });
//! app.status$;    // "idle" | "spawning" | "running" | "error" | "destroyed"
//! app.errorMsg$;
//! store.set(app.destroy$);                        // control mutation — the only lifecycle action
//!
//! VirtualAppView({ app$: app, background?, fallback?, errorView? });
//! ```
//!
//! The controller is a **lazy declaration** — nothing runs until an element
//! binds it (`app$` resolving to a controller); unbinding destroys the
//! child unless `keepAlive`.

mod bridge;
mod element;
mod handlers;
mod state;

pub(crate) use state::VirtualState;

use std::rc::Rc;

use crate::core::js_runtime::TurInstanceContext;
use crate::core::js_runtime::helpers::FnEntry;
use crate::core::plugin::PluginRegisterContext;
use crate::error::TurError;

/// Install the virtual-app bridge + subsystem into `tur:std`. The shared
/// per-instance `Rc<VirtualState>` rides the register-phase plugin-state
/// channel, so the three bridge fns are plain ctx-bound `FnEntry` pointers
/// (state reached via `args[0]` — no closure captures, no `unsafe`).
pub(crate) fn install_virtual_app(
    ctx: &mut PluginRegisterContext<'_>,
) -> Result<Vec<FnEntry>, TurError> {
    let js_ctx: TurInstanceContext = ctx.js_ctx().clone();
    let state = Rc::new(VirtualState::new(js_ctx.host_tx.clone(), ctx.reactive()));
    ctx.define_plugin_state::<VirtualState>(state.clone());
    ctx.register_subsystem(Box::new(handlers::VirtualAppSubsystem::new(state, js_ctx)));
    Ok(bridge::fns())
}
