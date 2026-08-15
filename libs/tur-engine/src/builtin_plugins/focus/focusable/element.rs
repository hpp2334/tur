use std::rc::Rc;

use boa_engine::Context;
use boa_engine::object::JsObject;

use crate::core::edgy::mutation::MutationHandle;
use crate::core::element::{ElementNodeId, NodeId};
use crate::core::elements::{AnyElement, ElementOnFocus, ElementTrace};
use crate::core::focus::{BlurEvent, FocusEvent, Focusable};
use crate::core::js_runtime::JsProps;
use crate::core::platform::key_event::{KeydownEvent, KeyupEvent};
use crate::core::view::{Lifecycle, View, ViewCx};

// ---------------------------------------------------------------------------
// FocusableView — wraps a child and provides keyboard / focus callbacks.
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct FocusableView {
    pub(crate) on_key_down: Option<MutationHandle<KeydownEvent>>,
    #[allow(dead_code)]
    pub(crate) on_key_up: Option<MutationHandle<KeyupEvent>>,
    pub(crate) on_focus: Option<MutationHandle<FocusEvent>>,
    pub(crate) on_blur: Option<MutationHandle<BlurEvent>>,
    pub(crate) child: Option<Rc<dyn View>>,
}

impl View for FocusableView {
    fn build(&self, cx: &mut dyn ViewCx, boa: &mut Context, parent: NodeId) -> NodeId {
        let id: ElementNodeId = cx.alloc_node().as_element_id();
        cx.insert_node(
            id,
            AnyElement::new(FocusableElement { view: self.clone() })
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
    fn on_focus_mutation(&self) -> Option<MutationHandle<FocusEvent>> {
        self.view.on_focus
    }

    fn on_blur_mutation(&self) -> Option<MutationHandle<BlurEvent>> {
        self.view.on_blur
    }
}

impl crate::core::layout::ElementSubscribe for FocusableElement {}

impl Lifecycle for FocusableElement {}

impl ElementTrace for FocusableElement {}

impl ElementOnFocus for FocusableElement {}

// ---------------------------------------------------------------------------
// Factory helpers
// ---------------------------------------------------------------------------

impl FocusableView {
    pub fn from_js(props: &JsObject, ctx: &mut Context) -> Self {
        let mut p = JsProps::new(props, ctx);
        FocusableView {
            on_key_down: p.mutation::<KeydownEvent>("onKeyDown"),
            on_key_up: p.mutation::<KeyupEvent>("onKeyUp"),
            on_focus: p.mutation::<FocusEvent>("onFocus"),
            on_blur: p.mutation::<BlurEvent>("onBlur"),
            child: p.child("child"),
        }
    }
}
