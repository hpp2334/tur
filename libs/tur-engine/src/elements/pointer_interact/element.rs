use std::rc::Rc;

use boa_engine::object::builtins::JsFunction;
use boa_engine::object::JsObject;
use boa_engine::{Context, JsValue};
use tur_shared::HitTestBehavior;

use crate::core::element::ElementNodeId;
use crate::core::elements::{AnyElement, ElementJsCallbackEmitter, ElementTrace};
use crate::core::js_command::{AnyJsCommand, PointerInteractJsCommand};
use crate::core::reactive::{extract_atom, AtomId};
use crate::core::widget::{
    extract_spec, make_mutation_callback, val_from_js, Effect, PropValue, Spec, Val, WidgetCx,
};

// ---------------------------------------------------------------------------
// PointerInteractSpec — the user's declaration. Pure Rust, no JsValues.
//
// Callbacks (`on_click`, `on_pointer_enter`, `on_pointer_exit`) are mutation
// atoms — the JS bridge wraps user callbacks as mutation atoms and passes the
// `AtomHandle` as the prop value.  During `build` these are turned into
// `JsFunction`s via `make_mutation_callback` and stored on the element.
//
// `behavior` is reactive (`Val<HitTestBehavior>`) but is resolved eagerly at
// build time so the gesture/pointer-region handlers can read it at event time
// without needing store access.
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct PointerInteractSpec {
    pub behavior: Option<Val<HitTestBehavior>>,
    pub on_click: Option<AtomId>,
    pub on_pointer_enter: Option<AtomId>,
    pub on_pointer_exit: Option<AtomId>,
    pub child: Option<Rc<dyn Spec>>,
}

impl Spec for PointerInteractSpec {
    fn build(&self, cx: &mut WidgetCx, boa: &mut Context, parent: ElementNodeId) -> ElementNodeId {
        // Resolve behavior eagerly — gesture/pointer-region handlers read it
        // at event time where no store/Context is available.
        let behavior = self
            .behavior
            .as_ref()
            .and_then(|v| cx.read_val(v, boa))
            .unwrap_or_default();

        // Turn mutation atoms into JS callback functions.
        let on_click = self.on_click.and_then(|id| make_mutation_callback(cx, boa, id));
        let on_pointer_enter = self
            .on_pointer_enter
            .and_then(|id| make_mutation_callback(cx, boa, id));
        let on_pointer_exit = self
            .on_pointer_exit
            .and_then(|id| make_mutation_callback(cx, boa, id));

        let id = cx.alloc_node();
        cx.insert_node(
            id,
            AnyElement::new(PointerInteract {
                spec: self.clone(),
                behavior,
                on_click,
                on_pointer_enter,
                on_pointer_exit,
            })
            .with_js_callback_emitter::<PointerInteract>(),
            boa,
        );
        if let Some(child) = &self.child {
            child.build(cx, boa, id);
        }
        cx.link_child(parent, id);
        id
    }
}

// ---------------------------------------------------------------------------
// PointerInteract — the built element. Wraps a single child and provides
// click / pointer-enter / pointer-exit callback emission.
// ---------------------------------------------------------------------------

pub struct PointerInteract {
    pub spec: PointerInteractSpec,
    behavior: HitTestBehavior,
    on_click: Option<JsFunction>,
    on_pointer_enter: Option<JsFunction>,
    on_pointer_exit: Option<JsFunction>,
}

impl PointerInteract {
    pub fn has_on_click(&self) -> bool {
        self.on_click.is_some()
    }

    pub fn has_pointer_region_callbacks(&self) -> bool {
        self.on_pointer_enter.is_some() || self.on_pointer_exit.is_some()
    }

    pub fn is_click_opaque(&self) -> bool {
        self.behavior == HitTestBehavior::Opaque && self.on_click.is_some()
    }

    pub fn is_pointer_region_opaque(&self) -> bool {
        self.behavior == HitTestBehavior::Opaque
            && (self.on_pointer_enter.is_some() || self.on_pointer_exit.is_some())
    }
}

impl Effect for PointerInteract {}

impl ElementTrace for PointerInteract {}

impl ElementJsCallbackEmitter for PointerInteract {
    fn emit_js_callback(
        &self,
        _context: &mut Context,
        command: AnyJsCommand,
    ) -> Option<(JsFunction, Vec<JsValue>)> {
        let c = command.downcast_ref::<PointerInteractJsCommand>()?;
        match c {
            PointerInteractJsCommand::Click { x, y } => self.on_click.as_ref().map(|h| {
                (h.clone(), vec![JsValue::from(*x), JsValue::from(*y)])
            }),
            PointerInteractJsCommand::PointerEnter => {
                self.on_pointer_enter.as_ref().map(|h| (h.clone(), vec![]))
            }
            PointerInteractJsCommand::PointerExit => {
                self.on_pointer_exit.as_ref().map(|h| (h.clone(), vec![]))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Factory — called from the JS bridge to parse props into a spec.
// ---------------------------------------------------------------------------

/// Extract a `Val<T>` prop from a JS props object.
fn prop_val<T: PropValue>(props: &JsObject, key: &str, ctx: &mut Context) -> Option<Val<T>> {
    use boa_engine::js_string;
    let v = props.get(js_string!(key), ctx).ok()?;
    val_from_js(&v)
}

/// Extract a mutation `AtomId` from a JS prop value.
fn prop_atom(props: &JsObject, key: &str, ctx: &mut Context) -> Option<AtomId> {
    use boa_engine::js_string;
    let v = props.get(js_string!(key), ctx).ok()?;
    extract_atom(&v)
}

/// Extract the single child spec from a JS props object.
fn prop_child(props: &JsObject, key: &str, ctx: &mut Context) -> Option<Rc<dyn Spec>> {
    use boa_engine::js_string;
    let v = props.get(js_string!(key), ctx).ok()?;
    extract_spec(&v)
}

impl PointerInteractSpec {
    pub fn from_js(props: &JsObject, ctx: &mut Context) -> Self {
        PointerInteractSpec {
            behavior: prop_val::<HitTestBehavior>(props, "behavior", ctx),
            on_click: prop_atom(props, "onClick", ctx),
            on_pointer_enter: prop_atom(props, "onPointerEnter", ctx),
            on_pointer_exit: prop_atom(props, "onPointerExit", ctx),
            child: prop_child(props, "child", ctx),
        }
    }
}
