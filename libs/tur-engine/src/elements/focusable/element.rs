use std::rc::Rc;

use boa_engine::object::JsObject;
use boa_engine::Context;

use crate::core::edgy_event::{edgy_mutation_from_js, EdgyMutation, EventArg};
use crate::core::element::ElementNodeId;
use crate::core::elements::{AnyElement, ElementOnFocus, ElementTrace};
use crate::core::focus::{BlurEvent, FocusEvent};
use crate::core::keyboard::{KeydownEvent, KeyupEvent};
use crate::core::widget::{extract_spec, Effect, Spec, WidgetCx};

// ---------------------------------------------------------------------------
// FocusableSpec — wraps a child and provides keyboard / focus callbacks.
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct FocusableSpec {
    pub on_key_down: Option<EdgyMutation<KeydownEvent>>,
    pub on_key_up: Option<EdgyMutation<KeyupEvent>>,
    pub on_focus: Option<EdgyMutation<FocusEvent>>,
    pub on_blur: Option<EdgyMutation<BlurEvent>>,
    pub child: Option<Rc<dyn Spec>>,
}

impl Spec for FocusableSpec {
    fn build(&self, cx: &mut WidgetCx, boa: &mut Context, parent: ElementNodeId) -> ElementNodeId {
        let id = cx.alloc_node();
        cx.insert_node(
            id,
            AnyElement::new(Focusable {
                spec: self.clone(),
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
// Focusable — the built element. Stores only the spec; mutations are
// resolved from the spec at push time and invoked via the reactive store.
// ---------------------------------------------------------------------------

pub struct Focusable {
    pub spec: FocusableSpec,
}

impl Effect for Focusable {}

impl ElementTrace for Focusable {}

impl ElementOnFocus for Focusable {}

// ---------------------------------------------------------------------------
// Factory helpers
// ---------------------------------------------------------------------------

fn prop_mutation<E: EventArg>(
    props: &JsObject,
    key: &str,
    ctx: &mut Context,
) -> Option<EdgyMutation<E>> {
    use boa_engine::js_string;
    let v = props.get(js_string!(key), ctx).ok()?;
    edgy_mutation_from_js(&v)
}

fn prop_child(props: &JsObject, key: &str, ctx: &mut Context) -> Option<Rc<dyn Spec>> {
    use boa_engine::js_string;
    let v = props.get(js_string!(key), ctx).ok()?;
    extract_spec(&v)
}

impl FocusableSpec {
    pub fn from_js(props: &JsObject, ctx: &mut Context) -> Self {
        FocusableSpec {
            on_key_down: prop_mutation::<KeydownEvent>(props, "onKeyDown", ctx),
            on_key_up: prop_mutation::<KeyupEvent>(props, "onKeyUp", ctx),
            on_focus: prop_mutation::<FocusEvent>(props, "onFocus", ctx),
            on_blur: prop_mutation::<BlurEvent>(props, "onBlur", ctx),
            child: prop_child(props, "child", ctx),
        }
    }
}
