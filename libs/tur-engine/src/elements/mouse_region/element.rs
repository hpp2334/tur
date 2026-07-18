use std::rc::Rc;

use boa_engine::object::JsObject;
use boa_engine::{Context, JsValue};
use crate::core::layout::{HitTestBehavior, Offset};
use crate::core::platform::{Cursor};

use crate::core::bridge::JsProps;
use crate::core::mutation::{MutationHandle, IntoJsArgs};
use crate::core::element::{ElementNodeId, NodeId};
use crate::core::elements::{AnyElement, ElementTrace, TraceValue};
use crate::core::layout::SubscribeCx;
use crate::core::view::{ViewCx, read_val, Lifecycle, Val, View};

// ---------------------------------------------------------------------------
// MouseRegionView — the user's declaration. Pure Rust, no JsValues.
//
// `cursor` is reactive (`Val<Cursor>`); it is resolved to a concrete `Cursor`
// during layout (where the JS engine is available) and read by the pointer-
// region handler at event time. `on_enter` / `on_exit` are mutation atoms
// invoked by the pointer-region handler when this region enters or leaves
// the hit-path.
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct MouseRegionView {
    pub(crate) behavior: Option<Val<HitTestBehavior>>,
    pub(crate) cursor: Option<Val<Cursor>>,
    pub on_enter: Option<MutationHandle<PointerRegionEvent>>,
    pub on_exit: Option<MutationHandle<PointerRegionEvent>>,
    pub(crate) child: Option<Rc<dyn View>>,
}

impl View for MouseRegionView {
    fn build(&self, cx: &mut dyn ViewCx, boa: &mut Context, parent: NodeId) -> NodeId {
        let behavior = self
            .behavior
            .as_ref()
            .and_then(|v| read_val(cx, v, boa))
            .unwrap_or_default();

        let id: ElementNodeId = ElementNodeId::new(cx.alloc_node().as_u64());
        cx.insert_node(
            id,
            AnyElement::new(MouseRegionElement {
                view: self.clone(),
                behavior,
                cursor: None,
            })
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
// MouseRegionElement — the built element. Stores spec + eagerly-resolved
// behavior (resolved at build) and the layout-resolved `cursor`. Both are
// read by the pointer-region handler at event time, where no store/Context is
// available.
// ---------------------------------------------------------------------------

pub struct MouseRegionElement {
    pub view: MouseRegionView,
    pub(crate) behavior: HitTestBehavior,
    pub(crate) cursor: Option<Cursor>,
}

impl MouseRegionElement {
    pub fn has_region_callbacks(&self) -> bool {
        self.view.on_enter.is_some() || self.view.on_exit.is_some()
    }

    pub fn has_cursor(&self) -> bool {
        self.view.cursor.is_some()
    }

    /// The layout-resolved cursor for this region, if any.
    pub fn resolved_cursor(&self) -> Option<Cursor> {
        self.cursor
    }

    pub fn is_region_opaque(&self) -> bool {
        self.behavior == HitTestBehavior::Opaque && self.has_region_callbacks()
    }
}

impl crate::core::layout::ElementSubscribe for MouseRegionElement {
    fn subscribe(&self, cx: &mut SubscribeCx) {
        if let Some(v) = self.view.cursor.as_ref() {
            cx.subscribe_val(v);
        }
    }
}

impl Lifecycle for MouseRegionElement {}

impl ElementTrace for MouseRegionElement {
    fn trace_props(&self) -> Vec<(&'static str, TraceValue)> {
        let mut p = vec![("behavior", TraceValue::Str(format!("{:?}", self.behavior)))];
        if let Some(c) = self.view.cursor.as_ref().and_then(Val::as_static) {
            p.push(("cursor", TraceValue::Str(c.as_str().to_string())));
        }
        p
    }
}

// ---------------------------------------------------------------------------
// Factory — called from the JS bridge to parse props into a spec.
// ---------------------------------------------------------------------------

impl MouseRegionView {
    pub fn from_js(props: &JsObject, ctx: &mut Context) -> Self {
        let mut p = JsProps::new(props, ctx);
        MouseRegionView {
            behavior: p.val::<HitTestBehavior>("behavior"),
            cursor: p.val::<Cursor>("cursor"),
            on_enter: p.mutation::<PointerRegionEvent>("onEnter"),
            on_exit: p.mutation::<PointerRegionEvent>("onExit"),
            child: p.child("child"),
        }
    }
}

// ---------------------------------------------------------------------------
// PointerRegionEvent — JS callback argument for `onEnter` / `onExit`.
// Serialises to a single JS object `{ local: {x, y}, global: {x, y} }`.
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct PointerRegionEvent {
    pub local: Offset,
    pub global: Offset,
}

impl IntoJsArgs for PointerRegionEvent {
    fn to_js_args(&self, ctx: &mut Context) -> Vec<JsValue> {
        use boa_engine::js_string;
        use boa_engine::object::JsObject;

        fn make_point(ctx: &mut Context, x: f64, y: f64) -> JsObject {
            let obj = JsObject::with_object_proto(ctx.intrinsics());
            let _ = obj.create_data_property(js_string!("x"), JsValue::from(x), ctx);
            let _ = obj.create_data_property(js_string!("y"), JsValue::from(y), ctx);
            obj
        }
        fn make_event(ctx: &mut Context, local: JsObject, global: JsObject) -> JsObject {
            let obj = JsObject::with_object_proto(ctx.intrinsics());
            let _ = obj.create_data_property(js_string!("local"), JsValue::from(local), ctx);
            let _ = obj.create_data_property(js_string!("global"), JsValue::from(global), ctx);
            obj
        }

        let local = make_point(ctx, self.local.x, self.local.y);
        let global = make_point(ctx, self.global.x, self.global.y);
        let event = make_event(ctx, local, global);
        vec![JsValue::from(event)]
    }
}
