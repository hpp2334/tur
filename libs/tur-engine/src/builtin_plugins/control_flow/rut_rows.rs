//! The control-flow families' `tur` host-pkg rows (via the pkg-extension
//! seam): Condition, Switch, Each (one child per item of a native list
//! atom), and Fragment.
//!
//! rut closures cannot cross the host boundary, so the Each item builder
//! is a kit-sealed fn box (`opaque(cb)`) invoked through the guarded VM
//! face ([`VmFace`]) via the kit's `__tur_cb_build2` dispatch entry — the
//! same flush-time-call law C8 formalizes for deriveds: fuel-capped,
//! depth-limited, no-mount-guarded, traps reported (never aborting the
//! flush). The item builder signature is
//! `fn item_fn(index: u64, item: str) -> opaque`.

use std::rc::Rc;

use crate::builtin_plugins::control_flow::each::{EachBuilder, EachView};
use crate::builtin_plugins::control_flow::{ConditionView, FragmentView, SwitchView};
use crate::core::edgy::reactive::{AnyReadable, AtomId, Derived, Readable, Source};
use crate::core::edgy::value::Value;
use crate::core::rut_runtime::{RutHandles, RutView, readable_of};
use crate::core::view::Val;

use rut_vm::{Opaque, OpaqueRef};

use super::Prebuilt;

/// The pkg-extension payload: decl rows at compile time, bodies at boot.
pub(crate) fn install_ext(cx: &mut crate::core::rut_runtime::RutPkgCx<'_>) {
    install_decl(cx);
    let Some(handles) = cx.handles else {
        return; // the compile-time decl probe — bodies install at boot only
    };
    let handles = handles.clone();
    install(&mut *cx.pkg, &handles);
}

/// Declare the control-flow rows (the installer's decl half).
pub fn install_decl(cx: &mut crate::core::rut_runtime::RutPkgCx<'_>) {
    let row = |n: &str, p: Vec<rut_core::types::TypeId>, r: rut_core::types::TypeId| {
        (n.to_string(), p, r, false)
    };
    cx.decl.extend(vec![
        // condition
        row("cond_new", vec![TY_U64], TY_OPAQUE),
        row("cond_then", vec![TY_OPAQUE, TY_OPAQUE], TY_NIL),
        row("cond_else", vec![TY_OPAQUE, TY_OPAQUE], TY_NIL),
        row("cond_qkey", vec![TY_OPAQUE, TY_STR], TY_NIL),
        row("cond_build", vec![TY_OPAQUE], TY_OPAQUE),
        // switch
        row("switch_new", vec![], TY_OPAQUE),
        row("switch_value_source", vec![TY_OPAQUE, TY_U64], TY_NIL),
        row("switch_value_derived", vec![TY_OPAQUE, TY_U64], TY_NIL),
        row("switch_case", vec![TY_OPAQUE, TY_STR, TY_OPAQUE], TY_NIL),
        row("switch_fallback", vec![TY_OPAQUE, TY_OPAQUE], TY_NIL),
        row("switch_qkey", vec![TY_OPAQUE, TY_STR], TY_NIL),
        row("switch_build", vec![TY_OPAQUE], TY_OPAQUE),
        // each
        row("each_new", vec![TY_U64], TY_OPAQUE),
        row("each_builder", vec![TY_OPAQUE, TY_OPAQUE], TY_NIL),
        row("each_build", vec![TY_OPAQUE], TY_OPAQUE),
        // fragment
        row("frag_new", vec![], TY_OPAQUE),
        row("frag_child", vec![TY_OPAQUE, TY_OPAQUE], TY_NIL),
        row("frag_build", vec![TY_OPAQUE], TY_OPAQUE),
    ]);
}

use rut_core::types::{TY_NIL, TY_OPAQUE, TY_STR, TY_U64};

/// The condition spec (both branches authored at start time; the swap is
/// pure engine — the factory clones a pre-built Rc, no rut during flush).
pub(crate) struct CondSpec {
    condition: Val<bool>,
    then_child: Option<Rc<dyn crate::core::view::View>>,
    else_child: Option<Rc<dyn crate::core::view::View>>,
    query_key: Option<Vec<String>>,
}

/// The Each spec (items atom + the entry-fn item builder).
pub(crate) struct EachSpec {
    items: Option<AnyReadable>,
    builder: Option<EachBuilder>,
}

/// Install the control-flow-row bodies (the installer's boot half).
pub fn install(pkg: &mut rut_vm::interp::HostPkg, handles: &Rc<RutHandles>) {
    // ---- condition ---------------------------------------------------------
    // conditional: both branches authored at start time; the swap is pure
    // engine (the factory clones a pre-built Rc — no rut during flush).
    rut_vm::pkg_fn!(pkg, "cond_new", (u64,) -> rut_vm::OpaqueRef, |vm: &mut rut_vm::interp::Vm, when: u64| {
        let spec = CondSpec {
            condition: Val::Reactive(readable_of::<bool>(when)),
            then_child: None,
            else_child: None,
            query_key: None,
        };
        Ok(Opaque::alloc(vm, spec)?.handle().clone())
    });
    rut_vm::pkg_fn!(pkg, "cond_then", (Opaque<CondSpec>, Opaque<RutView>) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<CondSpec>, child: Opaque<RutView>| {
        let child = child.with(|v| v.0.clone())?;
        b.with_mut(vm, |_vm, s| s.then_child = Some(child))
    });
    rut_vm::pkg_fn!(pkg, "cond_else", (Opaque<CondSpec>, Opaque<RutView>) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<CondSpec>, child: Opaque<RutView>| {
        let child = child.with(|v| v.0.clone())?;
        b.with_mut(vm, |_vm, s| s.else_child = Some(child))
    });
    rut_vm::pkg_fn!(pkg, "cond_qkey", (Opaque<CondSpec>, &str) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<CondSpec>, key: &str| {
        let key: Vec<String> = key.split('/').map(str::to_string).collect();
        b.with_mut(vm, |_vm, s| s.query_key = Some(key))
    });
    rut_vm::pkg_fn!(pkg, "cond_build", (Opaque<CondSpec>,) -> rut_vm::OpaqueRef, |vm: &mut rut_vm::interp::Vm, b: Opaque<CondSpec>| {
        let view = b.with(|s| {
            let then_v = s.then_child.clone().expect("cond_build: no then branch");
            let else_v = s.else_child.clone().expect("cond_build: no else branch");
            let mut view = ConditionView::new_rut(
                s.condition.clone(),
                Rc::new(Prebuilt(then_v)),
                Rc::new(Prebuilt(else_v)),
            );
            view.set_query_key(s.query_key.clone());
            Rc::new(view) as Rc<dyn crate::core::view::View>
        })?;
        Ok(Opaque::alloc(vm, RutView(view))?.handle().clone())
    });

    // ---- switch ------------------------------------------------------------
    rut_vm::pkg_fn!(pkg, "switch_new", () -> rut_vm::OpaqueRef, move |vm: &mut rut_vm::interp::Vm| {
        let view = SwitchView::new_rut(
            Val::Reactive(Readable::Source(Source::<crate::builtin_plugins::control_flow::SwitchKey>::from_id(AtomId(0)))),
            Vec::new(),
            None,
        );
        Ok(Opaque::alloc(vm, view)?.handle().clone())
    });
    rut_vm::pkg_fn!(pkg, "switch_value_source", (Opaque<SwitchView>, u64) -> (), move |vm: &mut rut_vm::interp::Vm, b: Opaque<SwitchView>, atom: u64| {
        b.with_mut(vm, |_vm, s| {
            s.set_value(Val::Reactive(Readable::Source(
                Source::<crate::builtin_plugins::control_flow::SwitchKey>::from_id(AtomId(atom as u32)),
            )));
        })
    });
    // The derived-value twin: the switch reads a Derived<SwitchKey> atom.
    rut_vm::pkg_fn!(pkg, "switch_value_derived", (Opaque<SwitchView>, u64) -> (), move |vm: &mut rut_vm::interp::Vm, b: Opaque<SwitchView>, atom: u64| {
        b.with_mut(vm, |_vm, s| {
            s.set_value(Val::Reactive(Readable::Derived(
                Derived::<crate::builtin_plugins::control_flow::SwitchKey>::from_id(AtomId(atom as u32)),
            )));
        })
    });
    rut_vm::pkg_fn!(pkg, "switch_case", (Opaque<SwitchView>, &str, Opaque<RutView>) -> (), move |vm: &mut rut_vm::interp::Vm, b: Opaque<SwitchView>, key: &str, child: Opaque<RutView>| {
        let child = child.with(|v| v.0.clone())?;
        let key = crate::builtin_plugins::control_flow::SwitchKey(Value::str(key));
        b.with_mut(vm, |_vm, s| s.push_case(key, Rc::new(Prebuilt(child))))
    });
    rut_vm::pkg_fn!(pkg, "switch_fallback", (Opaque<SwitchView>, Opaque<RutView>) -> (), move |vm: &mut rut_vm::interp::Vm, b: Opaque<SwitchView>, child: Opaque<RutView>| {
        let child = child.with(|v| v.0.clone())?;
        b.with_mut(vm, |_vm, s| s.set_fallback(Rc::new(Prebuilt(child))))
    });
    rut_vm::pkg_fn!(pkg, "switch_qkey", (Opaque<SwitchView>, &str) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<SwitchView>, key: &str| {
        let key: Vec<String> = key.split('/').map(str::to_string).collect();
        b.with_mut(vm, |_vm, s| s.set_query_key(key))
    });
    rut_vm::pkg_fn!(pkg, "switch_build", (Opaque<SwitchView>,) -> rut_vm::OpaqueRef, |vm: &mut rut_vm::interp::Vm, b: Opaque<SwitchView>| {
        let view = b.with(|s| Rc::new(s.clone()) as Rc<dyn crate::core::view::View>)?;
        Ok(Opaque::alloc(vm, RutView(view))?.handle().clone())
    });

    // ---- each ---------------------------------------------------------------
    // Each: one child per item of a list atom; the item builder is a
    // kit-sealed fn box `fn(index, item) -> opaque`, fired through the
    // kit's `__tur_cb_build2` dispatch entry.
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "each_new", (u64,) -> rut_vm::OpaqueRef, move |vm: &mut rut_vm::interp::Vm, atom: u64| {
        let _ = &h;
        let spec = EachSpec {
            items: Some(Readable::Source(
                Source::<Value>::from_id(AtomId(atom as u32)),
            )),
            builder: None,
        };
        Ok(Opaque::alloc(vm, spec)?.handle().clone())
    });
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "each_builder", (Opaque<EachSpec>, OpaqueRef) -> (), move |vm: &mut rut_vm::interp::Vm, b: Opaque<EachSpec>, cb: OpaqueRef| {
        let _ = vm;
        let builder = EachBuilder {
            cb,
            face: h.face.clone(),
            handles: h.clone(),
        };
        b.with_mut(vm, |_vm, s| s.builder = Some(builder))
    });
    rut_vm::pkg_fn!(pkg, "each_build", (Opaque<EachSpec>,) -> rut_vm::OpaqueRef, |vm: &mut rut_vm::interp::Vm, b: Opaque<EachSpec>| {
        let view = b.with(|s| {
            Rc::new(EachView::new_rut(
                s.items,
                Vec::new(),
                s.builder.clone().expect("each_build: no item builder"),
            )) as Rc<dyn crate::core::view::View>
        })?;
        Ok(Opaque::alloc(vm, RutView(view))?.handle().clone())
    });

    // ---- fragment ------------------------------------------------------------
    // layout-transparent group — children build directly under the parent
    // (the JS Fragment twin; keeps test-navigated trees flat).
    rut_vm::pkg_fn!(pkg, "frag_new", () -> rut_vm::OpaqueRef, |vm: &mut rut_vm::interp::Vm| {
        Ok(Opaque::alloc(vm, FragmentView::new_rut(Vec::new()))?.handle().clone())
    });
    rut_vm::pkg_fn!(pkg, "frag_child", (Opaque<FragmentView>, Opaque<RutView>) -> (), |vm: &mut rut_vm::interp::Vm, b: Opaque<FragmentView>, child: Opaque<RutView>| {
        let child = child.with(|v| v.0.clone())?;
        b.with_mut(vm, |_vm, s| s.push_child(child))
    });
    rut_vm::pkg_fn!(pkg, "frag_build", (Opaque<FragmentView>,) -> rut_vm::OpaqueRef, |vm: &mut rut_vm::interp::Vm, b: Opaque<FragmentView>| {
        let view = b.with(|s| Rc::new(s.clone()) as Rc<dyn crate::core::view::View>)?;
        Ok(Opaque::alloc(vm, RutView(view))?.handle().clone())
    });
}
