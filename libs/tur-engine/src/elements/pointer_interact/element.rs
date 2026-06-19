use std::rc::Rc;

use boa_engine::object::JsObject;
use boa_engine::{Context, JsValue};
use tur_shared::HitTestBehavior;

use crate::core::edgy_event::{edgy_mutation_from_js, EdgyMutation, EventArg};
use crate::core::element::ElementNodeId;
use crate::core::elements::{AnyElement, ElementTrace};
use crate::core::widget::{
    extract_component, val_from_js, Effect, PropValue, Component,
    Val, WidgetCx,
};

// ---------------------------------------------------------------------------
// PointerInteractComponent — the user's declaration. Pure Rust, no JsValues.
//
// Callbacks are mutation atoms typed as `EdgyMutation<E>`.  The JS bridge
// wraps user callbacks as mutation atoms and passes the `AtomHandle` as the
// prop value.  At event time the gesture / pointer-region handlers resolve
// these and push invocations onto the pending-mutation queue.
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct PointerInteractComponent {
    pub behavior: Option<Val<HitTestBehavior>>,
    pub on_click: Option<EdgyMutation<ClickEvent>>,
    pub on_pointer_enter: Option<EdgyMutation<PointerEnterEvent>>,
    pub on_pointer_exit: Option<EdgyMutation<PointerExitEvent>>,
    pub child: Option<Rc<dyn Component>>,
}

impl Component for PointerInteractComponent {
    fn build(&self, cx: &mut WidgetCx, boa: &mut Context, parent: ElementNodeId) -> ElementNodeId {
        let behavior = self
            .behavior
            .as_ref()
            .and_then(|v| cx.read_val(v, boa))
            .unwrap_or_default();

        let id = cx.alloc_node();
        cx.insert_node(
            id,
            AnyElement::new(PointerInteractElement {
                component: self.clone(),
                behavior,
            })
            .with_callbacks(),
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
// PointerInteractElement — the built element. Stores spec + eagerly-resolved
// behavior (read by gesture/pointer-region handlers at event time where no
// store/Context is available).
// ---------------------------------------------------------------------------

pub struct PointerInteractElement {
    pub component: PointerInteractComponent,
    behavior: HitTestBehavior,
}

impl PointerInteractElement {
    pub fn has_on_click(&self) -> bool {
        self.component.on_click.is_some()
    }

    pub fn has_pointer_region_callbacks(&self) -> bool {
        self.component.on_pointer_enter.is_some() || self.component.on_pointer_exit.is_some()
    }

    pub fn is_click_opaque(&self) -> bool {
        self.behavior == HitTestBehavior::Opaque && self.component.on_click.is_some()
    }

    pub fn is_pointer_region_opaque(&self) -> bool {
        self.behavior == HitTestBehavior::Opaque
            && (self.component.on_pointer_enter.is_some() || self.component.on_pointer_exit.is_some())
    }
}

impl Effect for PointerInteractElement {}

impl ElementTrace for PointerInteractElement {}

// ---------------------------------------------------------------------------
// Factory — called from the JS bridge to parse props into a spec.
// ---------------------------------------------------------------------------

fn prop_val<T: PropValue>(props: &JsObject, key: &str, ctx: &mut Context) -> Option<Val<T>> {
    use boa_engine::js_string;
    let v = props.get(js_string!(key), ctx).ok()?;
    val_from_js(&v)
}

fn prop_mutation<E: EventArg>(
    props: &JsObject,
    key: &str,
    ctx: &mut Context,
) -> Option<EdgyMutation<E>> {
    use boa_engine::js_string;
    let v = props.get(js_string!(key), ctx).ok()?;
    edgy_mutation_from_js(&v)
}

fn prop_child(props: &JsObject, key: &str, ctx: &mut Context) -> Option<Rc<dyn Component>> {
    use boa_engine::js_string;
    let v = props.get(js_string!(key), ctx).ok()?;
    extract_component(&v)
}

impl PointerInteractComponent {
    pub fn from_js(props: &JsObject, ctx: &mut Context) -> Self {
        PointerInteractComponent {
            behavior: prop_val::<HitTestBehavior>(props, "behavior", ctx),
            on_click: prop_mutation::<ClickEvent>(props, "onClick", ctx),
            on_pointer_enter: prop_mutation::<PointerEnterEvent>(props, "onPointerEnter", ctx),
            on_pointer_exit: prop_mutation::<PointerExitEvent>(props, "onPointerExit", ctx),
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
