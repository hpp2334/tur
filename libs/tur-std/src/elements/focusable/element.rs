use std::rc::Rc;

use boa_engine::object::JsObject;
use boa_engine::Context;

use tur_engine::core::edgy_event::{edgy_mutation_from_js, EdgyMutation, EventArg};
use tur_engine::core::element::{ElementNodeId, NodeId};
use tur_engine::core::elements::{AnyElement, ElementOnFocus, ElementTrace};
use tur_engine::core::focus::{BlurEvent, FocusEvent, Focusable};
use crate::keyboard::{KeydownEvent, KeyupEvent};
use tur_engine::core::view::{ViewCx, extract_view, Lifecycle, View};

// ---------------------------------------------------------------------------
// FocusableView — wraps a child and provides keyboard / focus callbacks.
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct FocusableView {
    pub(crate) on_key_down: Option<EdgyMutation<KeydownEvent>>,
    #[allow(dead_code)]
    pub(crate) on_key_up: Option<EdgyMutation<KeyupEvent>>,
    pub(crate) on_focus: Option<EdgyMutation<FocusEvent>>,
    pub(crate) on_blur: Option<EdgyMutation<BlurEvent>>,
    pub(crate) child: Option<Rc<dyn View>>,
}

impl View for FocusableView {
    fn build(&self, cx: &mut dyn ViewCx, boa: &mut Context, parent: NodeId) -> NodeId {
        let id: ElementNodeId = ElementNodeId::new(cx.alloc_node().as_u64());
        cx.insert_node(
            id,
            AnyElement::new(FocusableElement {
                view: self.clone(),
            })
            .with_focusable::<FocusableElement>()
            .with_callbacks(),
            boa,
        );
        if let Some(child) = &self.child {
            child.build(cx, boa, id.into());
        }
        cx.link_child(parent, id.into());
        id.into()
    }
}

// ---------------------------------------------------------------------------
// FocusableElement — the built element. Stores only the spec; mutations are
// resolved from the spec at push time and invoked via the reactive store.
// ---------------------------------------------------------------------------

pub struct FocusableElement {
    pub(crate) view: FocusableView,
}

impl Focusable for FocusableElement {
    fn on_focus_mutation(&self) -> Option<EdgyMutation<FocusEvent>> {
        self.view.on_focus
    }

    fn on_blur_mutation(&self) -> Option<EdgyMutation<BlurEvent>> {
        self.view.on_blur
    }
}

impl tur_engine::core::layout::ElementSubscribe for FocusableElement {}

impl Lifecycle for FocusableElement {}

impl ElementTrace for FocusableElement {}

impl ElementOnFocus for FocusableElement {}

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

fn prop_child(props: &JsObject, key: &str, ctx: &mut Context) -> Option<Rc<dyn View>> {
    use boa_engine::js_string;
    let v = props.get(js_string!(key), ctx).ok()?;
    extract_view(&v)
}

impl FocusableView {
    pub fn from_js(props: &JsObject, ctx: &mut Context) -> Self {
        FocusableView {
            on_key_down: prop_mutation::<KeydownEvent>(props, "onKeyDown", ctx),
            on_key_up: prop_mutation::<KeyupEvent>(props, "onKeyUp", ctx),
            on_focus: prop_mutation::<FocusEvent>(props, "onFocus", ctx),
            on_blur: prop_mutation::<BlurEvent>(props, "onBlur", ctx),
            child: prop_child(props, "child", ctx),
        }
    }
}
