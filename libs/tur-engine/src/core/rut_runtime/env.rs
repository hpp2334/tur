//! The engine-environment rows — the viewport atoms' ids.
//!
//! `TurStdPlugin::register` mints the two f64 backing sources
//! (`viewport_width` / `viewport_height`, CSS-pixel logical units) and
//! slots their ids as plugin state ([`ViewportEnv`]); these rows answer
//! the ids per instance so rut code can bind the SAME atoms the engine's
//! [`crate::core::screen::ResizeSubsystem`] republishes on every shell
//! resize. Read-only by construction: the atoms are engine-owned truth,
//! never written from rut.

use std::rc::Rc;

use rut_core::types::TY_U64;

use super::RutHandles;

/// The per-instance viewport env atoms (the register-phase plugin state
/// the `env` rows resolve). The ids address the f64 backing sources the
/// std plugin minted into the instance store.
pub struct ViewportEnv {
    /// The viewport width atom (CSS-pixel logical f64).
    pub w: u64,
    /// The viewport height atom (CSS-pixel logical f64).
    pub h: u64,
}

/// The plugin state the row bodies read — an absent state is a loud trap
/// (the std plugin registers it on every standard instance).
fn env_state(handles: &RutHandles) -> Result<Rc<ViewportEnv>, rut_vm::Trap> {
    handles.inst.plugin_state::<ViewportEnv>().ok_or_else(|| {
        rut_vm::Trap::new(
            rut_vm::TrapKind::Invalid,
            "env rows: the viewport atoms are not registered on this instance",
        )
    })
}

/// Declare the env rows (the decl half — module path `env`).
pub fn decl_rows() -> Vec<rut_driver::HostRow> {
    let row = |n: &str| {
        crate::core::rut_runtime::host_row_at("env", n, vec![], TY_U64, false)
    };
    vec![row("env_viewport_w"), row("env_viewport_h")]
}

/// Install the env-row bodies (the boot half).
pub fn install(pkg: &mut rut_vm::interp::HostPkg, handles: &Rc<RutHandles>) {
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "env::env_viewport_w", () -> u64, move |_vm: &mut rut_vm::interp::Vm| {
        Ok(env_state(&h)?.w)
    });
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "env::env_viewport_h", () -> u64, move |_vm: &mut rut_vm::interp::Vm| {
        Ok(env_state(&h)?.h)
    });
}
