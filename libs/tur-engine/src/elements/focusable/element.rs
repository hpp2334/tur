use std::rc::Rc;

use boa_engine::object::builtins::JsFunction;
use boa_engine::object::JsObject;
use boa_engine::{Context, JsValue};

use crate::core::element::ElementNodeId;
use crate::core::elements::{
    AnyElement, ElementJsCallbackEmitter, ElementOnFocus, ElementTrace,
};
use crate::core::js_command::{AnyJsCommand, FocusableJsCommand};
use crate::core::js_command::helpers::build_key_event_object;
use crate::core::reactive::AtomId;
use crate::core::widget::{
    extract_spec, make_mutation_callback, Effect, Spec, WidgetCx,
};

// ---------------------------------------------------------------------------
// FocusableSpec — wraps a child and provides keyboard / focus callbacks.
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct FocusableSpec {
    pub on_key_down: Option<AtomId>,
    pub on_key_up: Option<AtomId>,
    pub on_focus: Option<AtomId>,
    pub on_blur: Option<AtomId>,
    pub child: Option<Rc<dyn Spec>>,
}

impl Spec for FocusableSpec {
    fn build(&self, cx: &mut WidgetCx, boa: &mut Context, parent: ElementNodeId) -> ElementNodeId {
        let on_key_down = self.on_key_down.and_then(|id| make_mutation_callback(cx, boa, id));
        let on_key_up = self.on_key_up.and_then(|id| make_mutation_callback(cx, boa, id));
        let on_focus = self.on_focus.and_then(|id| make_mutation_callback(cx, boa, id));
        let on_blur = self.on_blur.and_then(|id| make_mutation_callback(cx, boa, id));

        let id = cx.alloc_node();
        cx.insert_node(
            id,
            AnyElement::new(Focusable {
                on_key_down,
                on_key_up,
                on_focus,
                on_blur,
            })
            .with_js_callback_emitter::<Focusable>(),
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
// Focusable — the built element.
// ---------------------------------------------------------------------------

pub struct Focusable {
    on_key_down: Option<JsFunction>,
    on_key_up: Option<JsFunction>,
    on_focus: Option<JsFunction>,
    on_blur: Option<JsFunction>,
}

impl Effect for Focusable {}

impl ElementTrace for Focusable {}

impl ElementOnFocus for Focusable {}

impl ElementJsCallbackEmitter for Focusable {
    fn emit_js_callback(
        &self,
        context: &mut Context,
        command: AnyJsCommand,
    ) -> Option<(JsFunction, Vec<JsValue>)> {
        let c = command.downcast_ref::<FocusableJsCommand>()?;
        match c {
            FocusableJsCommand::KeyDown { key, code, modifiers } => {
                self.on_key_down.as_ref().map(|h| {
                    let event_obj = build_key_event_object(key, code, modifiers, context);
                    (h.clone(), vec![event_obj])
                })
            }
            FocusableJsCommand::KeyUp { key, code, modifiers } => {
                self.on_key_up.as_ref().map(|h| {
                    let event_obj = build_key_event_object(key, code, modifiers, context);
                    (h.clone(), vec![event_obj])
                })
            }
            FocusableJsCommand::Focus => {
                self.on_focus.as_ref().map(|h| (h.clone(), vec![]))
            }
            FocusableJsCommand::Blur => {
                self.on_blur.as_ref().map(|h| (h.clone(), vec![]))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Factory helpers
// ---------------------------------------------------------------------------

fn prop_atom(props: &JsObject, key: &str, ctx: &mut Context) -> Option<AtomId> {
    use boa_engine::js_string;
    use crate::core::reactive::extract_atom;
    let v = props.get(js_string!(key), ctx).ok()?;
    extract_atom(&v)
}

fn prop_child(props: &JsObject, key: &str, ctx: &mut Context) -> Option<Rc<dyn Spec>> {
    use boa_engine::js_string;
    let v = props.get(js_string!(key), ctx).ok()?;
    extract_spec(&v)
}

impl FocusableSpec {
    pub fn from_js(props: &JsObject, ctx: &mut Context) -> Self {
        FocusableSpec {
            on_key_down: prop_atom(props, "onKeyDown", ctx),
            on_key_up: prop_atom(props, "onKeyUp", ctx),
            on_focus: prop_atom(props, "onFocus", ctx),
            on_blur: prop_atom(props, "onBlur", ctx),
            child: prop_child(props, "child", ctx),
        }
    }
}
