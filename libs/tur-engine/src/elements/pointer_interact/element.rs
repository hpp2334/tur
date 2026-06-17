use std::cell::RefCell;
use std::rc::Rc;

use boa_engine::object::JsObject;
use boa_engine::{Context, JsValue};
use tur_shared::HitTestBehavior;

use crate::core::element::ElementNodeId;
use crate::core::elements::{AnyElement, ElementJsCallbackEmitter, ElementTrace};
use crate::core::js_command::{AnyJsCommand, PointerInteractJsCommand};
use crate::core::reactive::Store;
use crate::core::widget::callback::Mutation;
use crate::core::widget::{
    callback::EventArg, extract_spec, make_mutation_callback, val_from_js, Effect, PropValue, Spec,
    Val, WidgetCx,
};

// ---------------------------------------------------------------------------
// PointerInteractSpec — the user's declaration. Pure Rust, no JsValues.
//
// Callbacks are mutation atoms typed as `Mutation<E>`.  The JS bridge wraps
// user callbacks as mutation atoms and passes the `AtomHandle` as the prop
// value.  At emit time these are resolved via `make_mutation_callback` using
// the store.
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct PointerInteractSpec {
    pub behavior: Option<Val<HitTestBehavior>>,
    pub on_click: Option<Mutation<ClickEvent>>,
    pub on_pointer_enter: Option<Mutation<PointerEnterEvent>>,
    pub on_pointer_exit: Option<Mutation<PointerExitEvent>>,
    pub child: Option<Rc<dyn Spec>>,
}

impl Spec for PointerInteractSpec {
    fn build(&self, cx: &mut WidgetCx, boa: &mut Context, parent: ElementNodeId) -> ElementNodeId {
        let behavior = self
            .behavior
            .as_ref()
            .and_then(|v| cx.read_val(v, boa))
            .unwrap_or_default();

        let id = cx.alloc_node();
        cx.insert_node(
            id,
            AnyElement::new(PointerInteract {
                spec: self.clone(),
                behavior,
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
// PointerInteract — the built element. Stores spec + eagerly-resolved
// behavior (read by gesture/pointer-region handlers at event time where no
// store/Context is available).
// ---------------------------------------------------------------------------

pub struct PointerInteract {
    pub spec: PointerInteractSpec,
    behavior: HitTestBehavior,
}

impl PointerInteract {
    pub fn has_on_click(&self) -> bool {
        self.spec.on_click.is_some()
    }

    pub fn has_pointer_region_callbacks(&self) -> bool {
        self.spec.on_pointer_enter.is_some() || self.spec.on_pointer_exit.is_some()
    }

    pub fn is_click_opaque(&self) -> bool {
        self.behavior == HitTestBehavior::Opaque && self.spec.on_click.is_some()
    }

    pub fn is_pointer_region_opaque(&self) -> bool {
        self.behavior == HitTestBehavior::Opaque
            && (self.spec.on_pointer_enter.is_some() || self.spec.on_pointer_exit.is_some())
    }
}

impl Effect for PointerInteract {}

impl ElementTrace for PointerInteract {}

impl ElementJsCallbackEmitter for PointerInteract {
    fn emit_js_callback(
        &self,
        context: &mut Context,
        store: &Rc<RefCell<Store>>,
        command: AnyJsCommand,
    ) -> Option<(boa_engine::object::builtins::JsFunction, Vec<JsValue>)> {
        let c = command.downcast_ref::<PointerInteractJsCommand>()?;
        match c {
            PointerInteractJsCommand::Click { x, y } => {
                let mutation = self.spec.on_click?;
                let func = make_mutation_callback(store, context, &mutation);
                let event = ClickEvent { x: *x, y: *y };
                Some((func, event.to_js_args(context)))
            }
            PointerInteractJsCommand::PointerEnter => {
                let mutation = self.spec.on_pointer_enter?;
                let func = make_mutation_callback(store, context, &mutation);
                Some((func, PointerEnterEvent.to_js_args(context)))
            }
            PointerInteractJsCommand::PointerExit => {
                let mutation = self.spec.on_pointer_exit?;
                let func = make_mutation_callback(store, context, &mutation);
                Some((func, PointerExitEvent.to_js_args(context)))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Factory — called from the JS bridge to parse props into a spec.
// ---------------------------------------------------------------------------

fn prop_val<T: PropValue>(props: &JsObject, key: &str, ctx: &mut Context) -> Option<Val<T>> {
    use boa_engine::js_string;
    let v = props.get(js_string!(key), ctx).ok()?;
    val_from_js(&v)
}

fn prop_mutation<E: EventArg, R: crate::core::widget::ReturnVal>(
    props: &JsObject,
    key: &str,
    ctx: &mut Context,
) -> Option<Mutation<E, R>> {
    use boa_engine::js_string;
    let v = props.get(js_string!(key), ctx).ok()?;
    crate::core::widget::mutation_from_js(&v)
}

fn prop_child(props: &JsObject, key: &str, ctx: &mut Context) -> Option<Rc<dyn Spec>> {
    use boa_engine::js_string;
    let v = props.get(js_string!(key), ctx).ok()?;
    extract_spec(&v)
}

impl PointerInteractSpec {
    pub fn from_js(props: &JsObject, ctx: &mut Context) -> Self {
        PointerInteractSpec {
            behavior: prop_val::<HitTestBehavior>(props, "behavior", ctx),
            on_click: prop_mutation::<ClickEvent, _>(props, "onClick", ctx),
            on_pointer_enter: prop_mutation::<PointerEnterEvent, _>(props, "onPointerEnter", ctx),
            on_pointer_exit: prop_mutation::<PointerExitEvent, _>(props, "onPointerExit", ctx),
            child: prop_child(props, "child", ctx),
        }
    }
}

// ---------------------------------------------------------------------------
// Pointer event payloads — JS callback arguments for click / enter / exit.
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct ClickEvent {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone)]
pub struct PointerEnterEvent;

#[derive(Clone)]
pub struct PointerExitEvent;

impl EventArg for ClickEvent {
    fn to_js_args(&self, _ctx: &mut Context) -> Vec<JsValue> {
        vec![JsValue::from(self.x), JsValue::from(self.y)]
    }
}

impl EventArg for PointerEnterEvent {
    fn to_js_args(&self, _ctx: &mut Context) -> Vec<JsValue> {
        Vec::new()
    }
}

impl EventArg for PointerExitEvent {
    fn to_js_args(&self, _ctx: &mut Context) -> Vec<JsValue> {
        Vec::new()
    }
}
