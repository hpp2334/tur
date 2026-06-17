use std::rc::Rc;

use boa_engine::object::JsObject;
use boa_engine::{Context, JsValue};

use crate::core::element::ElementNodeId;
use crate::core::elements::{
    AnyElement, ElementJsCallbackEmitter, ElementOnFocus, ElementTrace,
};
use crate::core::focus::{BlurEvent, FocusEvent, FocusableJsCommand};
use crate::core::js_command::AnyJsCommand;
use crate::core::keyboard::{KeydownEvent, KeyupEvent};
use crate::core::reactive::Store;
use crate::core::widget::callback::{EventArg, Mutation};
use crate::core::widget::{extract_spec, Effect, Spec, WidgetCx};

use std::cell::RefCell;

// ---------------------------------------------------------------------------
// FocusableSpec — wraps a child and provides keyboard / focus callbacks.
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct FocusableSpec {
    pub on_key_down: Option<Mutation<KeydownEvent>>,
    pub on_key_up: Option<Mutation<KeyupEvent>>,
    pub on_focus: Option<Mutation<FocusEvent>>,
    pub on_blur: Option<Mutation<BlurEvent>>,
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
// Focusable — the built element. Stores only the spec; mutations are
// resolved from the spec at emit time via the store.
// ---------------------------------------------------------------------------

pub struct Focusable {
    pub spec: FocusableSpec,
}

impl Effect for Focusable {}

impl ElementTrace for Focusable {}

impl ElementOnFocus for Focusable {}

impl ElementJsCallbackEmitter for Focusable {
    fn emit_js_callback(
        &self,
        context: &mut Context,
        store: &Rc<RefCell<Store>>,
        command: AnyJsCommand,
    ) -> Option<(boa_engine::object::builtins::JsFunction, Vec<JsValue>)> {
        let c = command.downcast_ref::<FocusableJsCommand>()?;
        match c {
            FocusableJsCommand::KeyDown { key, code, modifiers } => {
                let mutation = self.spec.on_key_down?;
                let func = crate::core::widget::make_mutation_callback(store, context, &mutation);
                let event = KeydownEvent {
                    key: key.clone(),
                    code: code.clone(),
                    modifiers: *modifiers,
                };
                Some((func, event.to_js_args(context)))
            }
            FocusableJsCommand::KeyUp { key, code, modifiers } => {
                let mutation = self.spec.on_key_up?;
                let func = crate::core::widget::make_mutation_callback(store, context, &mutation);
                let event = KeyupEvent {
                    key: key.clone(),
                    code: code.clone(),
                    modifiers: *modifiers,
                };
                Some((func, event.to_js_args(context)))
            }
            FocusableJsCommand::Focus => {
                let mutation = self.spec.on_focus?;
                let func = crate::core::widget::make_mutation_callback(store, context, &mutation);
                Some((func, FocusEvent.to_js_args(context)))
            }
            FocusableJsCommand::Blur => {
                let mutation = self.spec.on_blur?;
                let func = crate::core::widget::make_mutation_callback(store, context, &mutation);
                Some((func, BlurEvent.to_js_args(context)))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Factory helpers
// ---------------------------------------------------------------------------

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

impl FocusableSpec {
    pub fn from_js(props: &JsObject, ctx: &mut Context) -> Self {
        FocusableSpec {
            on_key_down: prop_mutation::<KeydownEvent, _>(props, "onKeyDown", ctx),
            on_key_up: prop_mutation::<KeyupEvent, _>(props, "onKeyUp", ctx),
            on_focus: prop_mutation::<FocusEvent, _>(props, "onFocus", ctx),
            on_blur: prop_mutation::<BlurEvent, _>(props, "onBlur", ctx),
            child: prop_child(props, "child", ctx),
        }
    }
}
