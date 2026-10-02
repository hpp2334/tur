//! Virtual apps, plugin half — `VirtualAppView` (the element that hosts a
//! complete nested engine instance) and the `VirtualAppSubsystem`
//! (status/frame consumption + layout-driven resize + input forwarding).
//!
//! The engine seam (host-side hosting, frame forwarding, the child shell)
//! lives in [`crate::core::virtual_app`]. Installed into the instance by
//! `TurStdPlugin` via [`install_virtual_app`] — the same `install_xxx(ctx)`
//! pattern as `install_text` / `install_scroll`.
//!
//! Authors mint module sources / controllers through the rut rows in
//! [`rut_rows`] (`va_create_source` / `va_source_handle` / `va_controller`
//! / `va_destroy`); the shared per-instance `Rc<VirtualState>` rides the
//! plugin-state channel.
//!
//! The controller is a **lazy declaration** — nothing runs until an element
//! binds it (`app$` resolving to a controller); unbinding destroys the
//! child unless `keepAlive`.

pub(crate) mod element;
pub(crate) mod rut_rows;
mod handlers;
pub mod state;

pub use state::VirtualState;

use std::rc::Rc;

use crate::core::plugin::PluginRegisterContext;
use crate::error::TurError;

/// Install the virtual-app subsystem. The shared per-instance
/// `Rc<VirtualState>` rides the register-phase plugin-state channel (read
/// back by the rut rows via `plugin_state::<VirtualState>()`).
pub(crate) fn install_virtual_app(ctx: &mut PluginRegisterContext) -> Result<(), TurError> {
    let instance = ctx.instance().clone();
    let state = Rc::new(VirtualState::new(instance.host_tx.clone(), ctx.reactive()));
    ctx.define_plugin_state::<VirtualState>(state.clone());
    ctx.register_subsystem(Box::new(handlers::VirtualAppSubsystem::new(state, instance)));
    // The virtual-app family's `tur` rows (the kit wraps them).
    ctx.push_rut_ext(std::rc::Rc::new(rut_rows::install_ext));
    Ok(())
}
