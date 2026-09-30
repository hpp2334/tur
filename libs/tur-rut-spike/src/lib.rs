//! Phase-0 spike: the rut scripting engine alongside tur-engine.
//!
//! Gates (from `.opencode/plan/boa-to-rut-migration.md`):
//! 1. rut builds on this workspace's pinned nightly, native + wasm32.
//! 2. Trapped work surfaces AT the embedder's call site
//!    (`vm.call` / `run_ready` return `Err(Trap { kind, msg })`) —
//!    the same shape tur's `RuntimeErrorReporter` consumes today.
//! 3. A host async fn via `register_async!` + `Completer`, driven to
//!    idle by the embedder's pump (`run_ready` + `pending_tasks`).
//! 4. Fuel exhaustion parks a call; `add_fuel` + `resume` continues it.
//! 5. Coexistence: rut + tur-engine (boa) linked into ONE binary,
//!    running in one process side by side.

use std::rc::Rc;
use std::sync::{Arc, Mutex};

pub const SPIKE_RUT: &str = include_str!("../spike.rut");
pub const HOSTPKG_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/hostpkg/spike");

/// The spike's booted VM plus the shared `spike::record` sink.
pub struct Spike {
    pub vm: rut_vm::interp::Vm,
    pub sink: Arc<Mutex<Vec<String>>>,
    pub fuel_limit: Option<u64>,
}

/// Mount core + the async pair + the spike host pkg, compile the spike
/// module, bind every declared row, verify the join, and boot the VM.
// (the needless_question_mark lint fires inside register_async!'s own
// row expansion — an upstream cosmetic, not this call site)
#[allow(clippy::needless_question_mark)]
pub fn boot(fuel: Option<u64>) -> Result<Spike, String> {
    let sink: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

    let mut session = rut_driver::Session::new();
    rut_driver::mount_std_core(&mut session);
    rut_driver::mount_std_async(&mut session);
    rut_driver::mount_dir(&mut session, std::path::Path::new(HOSTPKG_DIR))
        .map_err(|e| format!("mount spike hostpkg: {e}"))?;

    let out = rut_driver::compile_module_in(&mut session, SPIKE_RUT, rut_parser::Mode::Impl, "spike_main");
    if !out.diags.is_empty() {
        return Err(format!("compile diags: {:?}", out.diags));
    }
    let prog = rut_core::binary::decode(out.binary.as_deref().ok_or("no binary emitted")?)
        .map_err(|e| format!("binary decode: {e}"))?;
    rut_vm::verify::verify(&prog).map_err(|e| format!("verify: {e}"))?;

    let ctx = session.host_pkg_context();
    let mut hosts = rut_vm::interp::HostRegistry::new();
    // the launcher set (`__launch`/`__abort`/`__sleep`/`__sleep_yield`)
    hosts.install_host_pkg(&ctx, rut_std::async_host::pkg());

    let sink_for_record = sink.clone();
    rut_vm::register!(hosts, "spike::add", (i32, i32) -> i32,
        |_vm: &mut rut_vm::interp::Vm, a: i32, b: i32| -> Result<i32, rut_vm::Trap> {
            Ok(a + b)
        });
    rut_vm::register!(hosts, "spike::greet", (&str,) -> String,
        |_vm: &mut rut_vm::interp::Vm, name: &str| -> Result<String, rut_vm::Trap> {
            Ok(format!("hello, {name}!"))
        });
    rut_vm::register!(hosts, "spike::record", (&str,) -> (),
        move |_vm: &mut rut_vm::interp::Vm, line: &str| -> Result<(), rut_vm::Trap> {
            sink_for_record.lock().unwrap().push(line.to_string());
            Ok(())
        });
    rut_vm::register_async!(hosts, "spike::slow_echo", (u32, &str) -> String,
        |ms: u32, msg: &str| -> rut_vm::Completer<String> {
            let done = rut_vm::Completer::new();
            let w = done.clone();
            let msg = msg.to_string();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(ms as u64));
                w.complete(format!("echo:{msg}:{ms}"));
            });
            done
        },
        move |_c: rut_vm::Completer<String>| {
            // abort hook: the spike's sleep is not wire-cancellable —
            // the disclosed best-effort law (late answer discarded)
        });

    // the decl ↔ bodies join: panics loudly on any wiring bug
    hosts.verify_against(&ctx.flatten());

    let limits = rut_vm::interp::Limits {
        fuel,
        heap_limit_bytes: Some(64 * 1024 * 1024),
        interrupt_every: 1024,
    };
    let vm = rut_vm::interp::Vm::new(
        Rc::new(prog),
        &limits,
        rut_vm::interp::HostHooks::default(),
        hosts,
    )
    .map_err(|t| format!("boot: {} — {}", t.name(), t.msg))?;

    Ok(Spike { vm, sink, fuel_limit: fuel })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn sync_round_trip_typed_crossings() {
        let mut spike = boot(Some(1_000_000)).unwrap();
        let s: String = spike.vm.call("run", (2i32, 3i32)).unwrap();
        assert_eq!(s, "hello, sum=10!"); // add(2,3)=5, doubled
    }

    #[test]
    fn async_host_fn_drives_to_idle() {
        let mut spike = boot(Some(1_000_000)).unwrap();
        spike.vm.call::<_, ()>("boot", ("rust".to_string(), 5u32)).unwrap();

        let deadline = Instant::now() + Duration::from_secs(5);
        while spike.vm.pending_tasks() > 0 {
            spike.vm.run_ready().unwrap();
            assert!(Instant::now() < deadline, "async work never settled");
            std::thread::sleep(Duration::from_millis(2));
        }
        let sink = spike.sink.lock().unwrap();
        assert_eq!(sink.last().map(String::as_str), Some("echo:rust:5"));
    }

    #[test]
    fn trap_surfaces_at_the_embedder_call_site() {
        let mut spike = boot(Some(1_000_000)).unwrap();
        let err = spike.vm.call::<_, ()>("boom", ()).unwrap_err();
        assert_eq!(err.kind, rut_vm::TrapKind::Panic);
        assert!(err.msg.contains("boom!"), "trap msg: {}", err.msg);
    }

    #[test]
    fn fuel_parks_and_resumes() {
        let mut spike = boot(Some(1_000)).unwrap();
        let err = spike.vm.call::<_, i32>("spin", (10_000i32,)).unwrap_err();
        assert_eq!(err.kind, rut_vm::TrapKind::OutOfFuel);

        spike.vm.add_fuel(10_000_000);
        let answer: i32 = spike.vm.resume().unwrap();
        assert_eq!(answer, 49_995_000);
    }

    /// Gate 5: boa (tur-engine) and rut linked into one binary, running
    /// in one process. `TurTestApp::new` spins the whole tur runtime —
    /// scheduler pools, JS realm, renderer; the rut VM then runs its
    /// full round trip beside it.
    #[test]
    fn coexistence_with_tur_engine() {
        let app = tur_integration_tests::TurTestApp::new(200.0, 200.0).unwrap();
        app.wait_for_timeout(Duration::ZERO);

        let mut spike = boot(Some(1_000_000)).unwrap();
        let s: String = spike.vm.call("run", (1i32, 2i32)).unwrap();
        assert_eq!(s, "hello, sum=6!");

        // and the tur side still pumps after rut ran
        app.wait_for_timeout(Duration::ZERO);
    }
}
