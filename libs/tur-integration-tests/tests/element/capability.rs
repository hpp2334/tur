//! Verify the `Capability` registry: round-trip register → look up from a
//! plugin, from a bridge fn, from an event handler, and the build-time
//! `requires` validation.

use std::rc::Rc;
use std::sync::{Arc, Mutex};

use tur_engine::TurRuntime;
use tur_engine::TurStdPlugin;
use tur_engine::core::capability::{Capability, CapabilityDecls};
use tur_engine::core::plugin::{Plugin, PluginContext};
use tur_engine::error::TurError;
use tur_native::NativeFontLoader;

// ---------- Test capability newtype ---------------------------------------

#[derive(Clone, Debug)]
struct CountersCapability {
    value: Arc<Mutex<u32>>,
}

impl CountersCapability {
    fn new() -> Self {
        Self {
            value: Arc::new(Mutex::new(0)),
        }
    }
}

impl Capability for CountersCapability {}

// ---------- Plugin that requires the capability ---------------------------

struct NeedsCounterPlugin;
impl Plugin for NeedsCounterPlugin {
    fn requires(&self, decls: &mut CapabilityDecls) {
        decls.need::<CountersCapability>();
    }
    fn register(&self, _ctx: &mut PluginContext<'_>) -> Result<(), TurError> {
        Ok(())
    }
}

// Round-trip: register a capability on the builder, look it up from a
// plugin's `register`, and confirm the lookup sees the registered value.
#[test]
fn capability_round_trip_via_plugin() {
    let counter = CountersCapability::new();
    let captured = counter.value.clone();

    struct CapturePlugin {
        seen: Arc<Mutex<Option<CountersCapability>>>,
    }
    impl Plugin for CapturePlugin {
        fn register(&self, ctx: &mut PluginContext<'_>) -> Result<(), TurError> {
            *self.seen.lock().unwrap() = ctx.capability().of::<CountersCapability>();
            Ok(())
        }
    }

    let seen = Arc::new(Mutex::new(None));
    let app = build_app(|b| {
        b.capability(counter.clone())
            .plugin(TurStdPlugin)
            .plugin(CapturePlugin { seen: seen.clone() })
    });
    let got = seen
        .lock()
        .unwrap()
        .clone()
        .expect("plugin should see capability");
    // Phase 7: `captured` and `got.value` are clones of the SAME
    // `Arc<Mutex<u32>>` (the capability registry handed out an Arc clone,
    // not a deep copy). Locking the same Mutex twice in one expression
    // deadlocks, so compare Arc pointers instead.
    assert!(
        Arc::ptr_eq(&got.value, &captured),
        "plugin should see the same Arc-backed counter"
    );
    assert!(app.is_ok());
    drop(app);
}

// Build-time validation: a plugin declaring `requires::<CountersCapability>`
// against a builder with no matching `.capability(...)` call must fail with
// a clear error naming the missing type.
#[test]
fn missing_capability_fails_build() {
    let result = build_app(|b| b.plugin(TurStdPlugin).plugin(NeedsCounterPlugin));
    let err = result.err().expect("build must fail");
    let msg = err.to_string();
    assert!(
        msg.contains("CountersCapability"),
        "error should name the missing capability type, got: {msg}"
    );
    assert!(
        msg.contains(".capability("),
        "error should hint the fix (add `.capability(...)`), got: {msg}"
    );
}

// `requires` validation must pass when the capability is registered, even
// if the `.capability(...)` call comes AFTER the `.plugin(...)` call on the
// builder chain — order on the chain is irrelevant.
#[test]
fn capability_chain_order_irrelevant() {
    let app = build_app(|b| {
        b.plugin(TurStdPlugin)
            .plugin(NeedsCounterPlugin)
            .capability(CountersCapability::new())
    });
    assert!(
        app.is_ok(),
        "capability registered after plugin should still satisfy requires"
    );
}

// Helper: build a headless TurApp instance with a custom builder closure.
// Uses NativeFontLoader + FixedClock to match the test harness.
fn build_app(
    configure: impl FnOnce(tur_engine::TurRuntimeBuilder) -> tur_engine::TurRuntimeBuilder,
) -> Result<Rc<tur_engine::TurApp>, TurError> {
    use boa_engine::context::time::FixedClock;
    let builder = TurRuntime::builder()
        .font_loader(Rc::new(NativeFontLoader::new()))
        .clock(Rc::new(FixedClock::from_millis(0)));
    let runtime = configure(builder).build()?;
    let app = runtime.create_headless_app((400.0, 600.0))?;
    Ok(app)
}
