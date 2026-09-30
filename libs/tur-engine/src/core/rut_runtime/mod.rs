//! The rut runtime seam — the boa replacement's Phase-1 vertical slice.
//!
//! Architecture: **rut drives, the engine applies.** A loaded rut module's
//! `start()` builds a tree of *pure-Rust view data* through host rows
//! (element builders materialize `Rc<dyn View>` values directly — no boa,
//! no `JsValue` anywhere in the rut-built tree) and stashes the root via
//! `tur::mount`. The engine applies the stashed root into the instance's
//! `ElementTree` right after `start` returns, on the same code path the JS
//! `mount(view)` bridge uses. The rut VM itself is driven by the embedder's
//! pump (`run_ready()` before each flush — never inside a flush iteration,
//! so rut rows never race the flush's boa borrow).
//!
//! Module lifecycle contract (mirrors the JS contract): `entry fn start()`
//! is invoked after boot; `entry fn stop()` — if present — runs (best-effort)
//! before the next load and at destroy; the engine owns root-tree teardown.

use std::rc::Rc;

use boa_engine::{Context, JsValue};

use crate::core::app::root::RootView;
use crate::core::edgy::reactive::{AtomId, Readable, Source};
use crate::core::js_runtime::TurInstanceContext;
use crate::core::layout::Axis;
use crate::core::view::{SharedViewCx, View, Val};
use crate::builtin_plugins::layout::FlexView;
use crate::builtin_plugins::text::TextView;
use rut_core::types::{TypeId, TY_F64, TY_NIL, TY_OPAQUE, TY_STR, TY_U64};
use rut_driver::ModuleBody;
use rut_vm::Opaque;

/// The per-instance resource budget. Phase-1 defaults; tunable per embedder.
pub fn default_limits() -> rut_vm::interp::Limits {
    rut_vm::interp::Limits {
        fuel: Some(50_000_000),
        heap_limit_bytes: Some(64 * 1024 * 1024),
        interrupt_every: 1024,
    }
}

// ---------------------------------------------------------------------------
// View payloads — the opaque boxes rut rows mint and pass.
// ---------------------------------------------------------------------------

/// A materialized view (`Rc<dyn View>`) sealed in an opaque box.
pub struct RutView(pub Rc<dyn View>);

/// A builder under construction — materialized by `tur::el_build`.
enum ViewBuilder {
    Flex { axis: Axis, children: Vec<Rc<dyn View>> },
}

impl ViewBuilder {
    fn materialize(self) -> Rc<dyn View> {
        match self {
            ViewBuilder::Flex { axis, children } => Rc::new(FlexView {
                direction: Some(axis),
                main_alignment: None,
                cross_alignment: None,
                main_axis_size: None,
                children,
                query_key: None,
            }),
        }
    }
}

// ---------------------------------------------------------------------------
// The `tur` host package — decl rows (mounted in-memory as a Module) +
// bodies (a HostPkg installed into the per-instance HostRegistry).
// ---------------------------------------------------------------------------

/// The in-memory `tur` host-pkg Module (the DECL side): the surface rut
/// code compiles against. Mounted via `Session::register_module` — no
/// filesystem involved.
pub fn tur_decl_module() -> rut_driver::Module {
    let row = |name: &str, params: Vec<TypeId>, ret: TypeId| (name.to_string(), params, ret, false);
    let host_funcs: Vec<(String, Vec<TypeId>, TypeId, bool)> = vec![
        row("el_column", vec![], TY_OPAQUE),
        row("el_row", vec![], TY_OPAQUE),
        row("el_text", vec![TY_STR], TY_OPAQUE),
        row("el_text_bound", vec![TY_U64], TY_OPAQUE),
        row("el_build", vec![TY_OPAQUE], TY_OPAQUE),
        row("el_child", vec![TY_OPAQUE, TY_OPAQUE], TY_NIL),
        row("mount", vec![TY_OPAQUE], TY_NIL),
        row("rs_source_str", vec![TY_STR], TY_U64),
        row("rs_set_str", vec![TY_U64, TY_STR], TY_NIL),
        row("rs_source_f64", vec![], TY_U64),
        row("rs_set_f64", vec![TY_U64, TY_F64], TY_NIL),
    ];
    rut_driver::Module {
        namespace: Some("tur".to_string()),
        body: ModuleBody::Host {
            host_funcs,
            consts: Vec::new(),
            native_types: Vec::new(),
            native_traits: Vec::new(),
            native_fns: Vec::new(),
            native_impls: Vec::new(),
        },
        ..Default::default()
    }
}

/// The bodies. `handles` is the per-instance bridge state the rows close
/// over (the pending root stash; the dispatch registry + reactive rails
/// arrive in later phases).
fn install_tur_pkg(
    hosts: &mut rut_vm::interp::HostRegistry,
    ctx: &rut_vm::interp::HostPkgContext,
    handles: &Rc<RutHandles>,
) {
    let mut pkg = rut_vm::interp::HostPkg::new("tur");

    // mint a flex builder
    rut_vm::pkg_fn!(pkg, "el_column", () -> rut_vm::OpaqueRef, |vm: &mut rut_vm::interp::Vm| {
        Ok(Opaque::alloc(vm, ViewBuilder::Flex { axis: Axis::Vertical, children: Vec::new() })?.handle().clone())
    });
    rut_vm::pkg_fn!(pkg, "el_row", () -> rut_vm::OpaqueRef, |vm: &mut rut_vm::interp::Vm| {
        Ok(Opaque::alloc(vm, ViewBuilder::Flex { axis: Axis::Horizontal, children: Vec::new() })?.handle().clone())
    });
    // static text
    rut_vm::pkg_fn!(pkg, "el_text", (&str,) -> rut_vm::OpaqueRef, |vm: &mut rut_vm::interp::Vm, content: &str| {
        let view = Rc::new(TextView {
            text: Some(Val::Static(content.to_string())),
            font_size: None,
            font_weight: None,
            color: None,
            spans: None,
            query_key: None,
            on_selection_change: None,
            selectable: false,
            max_lines: None,
            overflow: None,
        });
        Ok(Opaque::alloc(vm, RutView(view))?.handle().clone())
    });
    // builder -> materialized view
    rut_vm::pkg_fn!(pkg, "el_build", (Opaque<ViewBuilder>,) -> rut_vm::OpaqueRef, |vm: &mut rut_vm::interp::Vm, b: Opaque<ViewBuilder>| {
        // `with` lends the payload — swap out a dummy to consume the builder
        let built = b.with_mut(vm, |_vm, b| {
            let dummy = ViewBuilder::Flex { axis: Axis::Vertical, children: Vec::new() };
            std::mem::replace(b, dummy).materialize()
        })?;
        Ok(Opaque::alloc(vm, RutView(built))?.handle().clone())
    });
    // attach a materialized child to a flex builder
    rut_vm::pkg_fn!(pkg, "el_child", (Opaque<ViewBuilder>, Opaque<RutView>) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<ViewBuilder>, child: Opaque<RutView>| {
        let child_view = child.with(|v| v.0.clone())?;
        b.with_mut(vm, |_vm, b| {
            // The only builder kind today; the match keeps future kinds honest.
            #[allow(irrefutable_let_patterns)]
            if let ViewBuilder::Flex { children, .. } = b {
                children.push(child_view);
            }
        })?;
        Ok(())
    });
    // stash the root — the engine applies it after `start` returns
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "mount", (Opaque<RutView>,) -> (), move |vm: &mut rut_vm::interp::Vm, view: Opaque<RutView>| {
        let root = view.with(|v| v.0.clone())?;
        let _ = vm;
        *h.pending_root.borrow_mut() = Some(root);
        Ok(())
    });

    // ---- reactive rails (Phase 2) --------------------------------------
    //
    // Atoms are the engine's edgy store (`core::edgy`) — the SAME KV the
    // JS realm and the element tree use — addressed by raw `AtomId` as
    // u64. Writes cross; reads are served by the rut-side mirror (the
    // wrapper atoms hold their current value), so no read path ever
    // needs the boa context. The flush fixed-point (stale atoms → dirty
    // subscribers → re-layout) is entirely the engine's existing
    // machinery: a bound Text re-renders on `rs_set_*` with zero new
    // engine code.
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "rs_source_str", (&str,) -> u64, move |vm: &mut rut_vm::interp::Vm, v: &str| {
        let _ = vm;
        let s: Source<JsValue> = h.store.bridge().decl_source(JsValue::from(boa_engine::js_string!(v)));
        Ok(s.id().0 as u64)
    });
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "rs_set_str", (u64, &str) -> (), move |vm: &mut rut_vm::interp::Vm, atom: u64, v: &str| {
        let _ = vm;
        h.store
            .bridge()
            .set_source(Source::<JsValue>::from_id(AtomId(atom as u32)), JsValue::from(boa_engine::js_string!(v)))
            .map_err(|e| rut_vm::Trap::new(rut_vm::TrapKind::Invalid, format!("rs_set_str: {e}")))
    });
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "rs_source_f64", () -> u64, move |vm: &mut rut_vm::interp::Vm| {
        let _ = vm;
        let s: Source<JsValue> = h.store.bridge().decl_source(JsValue::from(0.0f64));
        Ok(s.id().0 as u64)
    });
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "rs_set_f64", (u64, f64) -> (), move |vm: &mut rut_vm::interp::Vm, atom: u64, v: f64| {
        let _ = vm;
        h.store
            .bridge()
            .set_source(Source::<JsValue>::from_id(AtomId(atom as u32)), JsValue::from(boa_engine::js_string!(v)))
            .map_err(|e| rut_vm::Trap::new(rut_vm::TrapKind::Invalid, format!("rs_set_f64: {e}")))
    });
    // a Text bound to a str atom — re-renders when the atom changes
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "el_text_bound", (u64,) -> rut_vm::OpaqueRef, move |vm: &mut rut_vm::interp::Vm, atom: u64| {
        let _ = &h;
        let view = Rc::new(TextView {
            text: Some(Val::Reactive(Readable::Source(Source::<String>::from_id(AtomId(atom as u32))))),
            font_size: None,
            font_weight: None,
            color: None,
            spans: None,
            query_key: Some(vec!["rut".to_string(), "text".to_string()]),
            on_selection_change: None,
            selectable: false,
            max_lines: None,
            overflow: None,
        });
        Ok(Opaque::alloc(vm, RutView(view))?.handle().clone())
    });

    hosts.install_host_pkg(ctx, pkg);
}

// ---------------------------------------------------------------------------
// RutHandles — per-instance bridge state shared with the row closures.
// ---------------------------------------------------------------------------

pub struct RutHandles {
    /// The instance's reactive store — the SAME KV the JS realm and the
    /// element tree share; rut atoms are edgy atoms addressed by raw id.
    pub store: crate::core::edgy::reactive::Store,
    /// The instance-owned tree handle (a cheap clone of the one the JS
    /// realm shares) — `apply_root` builds into it.
    pub element_tree: crate::core::elements::NodeTree,
    /// The root stashed by `tur::mount` during `start`, applied by the
    /// engine after the call returns (outside the VM, on the mount path).
    pub pending_root: std::cell::RefCell<Option<Rc<dyn View>>>,
}

// ---------------------------------------------------------------------------
// RutRuntime — one per instance, lives beside the JS realm on the worker.
// ---------------------------------------------------------------------------

pub struct RutRuntime {
    pub vm: rut_vm::interp::Vm,
    handles: Rc<RutHandles>,
    /// The instance context (held for `apply_root` and future rails).
    js_ctx: TurInstanceContext,
    pub has_stop: bool,
    /// `entry fn start()`'s answer when declared `-> u64` (the module's
    /// handle back to the host — e.g. the id of its root atom), else 0.
    pub start_answer: u64,
}

impl RutRuntime {
    /// Assemble a fresh session (core + the in-memory `tur` decl pkg) and
    /// compile `source` against it. Split from [`Self::boot`] so a
    /// syntactically-broken module fails BEFORE any teardown runs (the
    /// parse-first contract).
    fn compile(
        source: &str,
    ) -> Result<(Rc<rut_core::binary::Program>, rut_vm::interp::HostPkgContext), String> {
        let mut session = rut_driver::Session::new();
        rut_driver::mount_std_core(&mut session);
        session
            .register_module("tur", tur_decl_module())
            .map_err(|e| format!("mount tur pkg: {e}"))?;

        let out = rut_driver::compile_module_in(&mut session, source, rut_parser::Mode::Impl, "app");
        if !out.diags.is_empty() {
            let msgs: Vec<String> = out.diags.iter().map(|d| d.msg.clone()).collect();
            return Err(msgs.join("; "));
        }
        let binary = out.binary.ok_or("rut compile emitted no binary")?;
        let prog = rut_core::binary::decode(&binary).map_err(|e| format!("decode: {e}"))?;
        rut_vm::verify::verify(&prog).map_err(|e| format!("verify: {e}"))?;
        Ok((Rc::new(prog), session.host_pkg_context()))
    }

    /// Parse + compile only (the parse-first half of the load contract) —
    /// a broken reload must fail before any teardown runs.
    pub fn parse_check(source: &str) -> Result<(), crate::core::app::ModuleError> {
        Self::compile(source)
            .map(|_| ())
            .map_err(crate::core::app::ModuleError::Parse)
    }

    /// Bind bodies, verify the join, boot the VM, and invoke `start`.
    pub fn boot(
        source: &str,
        js_ctx: TurInstanceContext,
    ) -> Result<Self, crate::core::app::ModuleError> {
        let (prog, ctx) = Self::compile(source).map_err(crate::core::app::ModuleError::Parse)?;

        let handles: Rc<RutHandles> = Rc::new(RutHandles {
            store: js_ctx.store.clone(),
            element_tree: js_ctx.element_tree.clone(),
            pending_root: std::cell::RefCell::new(None),
        });

        let mut hosts = rut_vm::interp::HostRegistry::new();
        install_tur_pkg(&mut hosts, &ctx, &handles);
        hosts.verify_against(&ctx.flatten());

        let export_of = |name: &str| {
            prog.exports
                .iter()
                .find(|(n, _)| prog.interner.name(*n) == name)
                .map(|(_, fid)| *fid as usize)
        };
        let has_stop = export_of("stop").is_some();
        // `entry fn start() -> u64` hands the host a module answer (e.g.
        // its root atom's id); a plain `start()` returns nil.
        let start_returns_u64 = export_of("start")
            .and_then(|fid| prog.funcs.get(fid))
            .is_some_and(|f| f.ret == TY_U64);

        let vm = rut_vm::interp::Vm::new(
            prog,
            &default_limits(),
            rut_vm::interp::HostHooks::default(),
            hosts,
        )
        .map_err(|t| crate::core::app::ModuleError::Eval(format!("boot: {} — {}", t.name(), t.msg)))?;

        let mut rt = RutRuntime {
            vm,
            handles,
            js_ctx: js_ctx.clone(),
            has_stop,
            start_answer: 0,
        };
        rt.call_start(start_returns_u64)?;
        Ok(rt)
    }

    fn call_start(&mut self, returns_u64: bool) -> Result<(), crate::core::app::ModuleError> {
        if returns_u64 {
            let answer = self
                .vm
                .call::<_, u64>("start", ())
                .map_err(|t| {
                    crate::core::app::ModuleError::Eval(format!("start: {} — {}", t.name(), t.msg))
                })?;
            self.start_answer = answer;
        } else {
            self.vm
                .call::<_, ()>("start", ())
                .map_err(|t| {
                    crate::core::app::ModuleError::Eval(format!("start: {} — {}", t.name(), t.msg))
                })?;
        }
        Ok(())
    }

    /// Call a named `entry fn(u64, f64)` — the engine→rut event rail
    /// (input dispatch, embedder events). Runs OUTSIDE flush; rows must
    /// not mount (stash-and-apply is a `start`-time contract in Phase 2).
    pub fn call_entry(&mut self, name: &str, a: u64, b: f64) -> Result<(), crate::core::app::ModuleError> {
        self.vm
            .call::<_, ()>(name, (a, b))
            .map(|_| ())
            .map_err(|t| crate::core::app::ModuleError::Eval(format!("{name}: {} — {}", t.name(), t.msg)))
    }

    /// Apply the root stashed by `tur::mount` into the instance tree —
    /// the engine-side twin of the JS `mount(view)` bridge. Runs with the
    /// caller's boa borrow (never inside the VM).
    pub fn apply_root(&mut self, boa: &mut Context) -> Result<(), String> {
        let Some(user_view) = self.handles.pending_root.borrow_mut().take() else {
            return Ok(());
        };
        let tree = self.handles.element_tree.clone();

        // One-root invariant: replace any existing root (same as JS mount).
        if let Some(old) = tree.borrow().root_element_id() {
            tree.borrow_mut().destroy_subtree(old);
        }

        let root_view = RootView { child: user_view };
        let mut cx = SharedViewCx::new(self.js_ctx.clone());
        let temp_parent = cx.alloc_node();
        let root_id = root_view.build(&mut cx, boa, temp_parent);
        tree.borrow_mut()
            .set_root_element(crate::core::element::ElementNodeId::new(root_id.as_u64()));
        Ok(())
    }

    /// Drive ready rut tasks once (pump-level — never inside a flush).
    pub fn run_ready(&mut self) {
        if let Err(t) = self.vm.run_ready() {
            tracing::error!("rut run_ready trap: {} — {}", t.name(), t.msg);
        }
    }

    /// Best-effort `entry fn stop()` (the cleanup contract).
    pub fn stop(&mut self) {
        if self.has_stop
            && let Err(t) = self.vm.call::<_, ()>("stop", ())
        {
            tracing::error!("rut module stop: {} — {}", t.name(), t.msg);
        }
        // Root teardown is engine-owned (teardown_current_module clears it).
        self.handles.pending_root.borrow_mut().take();
    }
}
