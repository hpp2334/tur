use std::rc::Rc;

use boa_engine::object::JsObject;
use boa_engine::Context;
use tur_shared::{ComputedLayout, Constraints, Offset, Size};
use vello::kurbo::Affine;

use crate::core::element::ElementNodeId;
use crate::core::elements::{AnyElement, ElementTrace};
use crate::core::layout::{ElementLayout, LayoutContext};
use crate::core::render::{Canvas, ElementRender, PaintContext};
use crate::core::widget::{
    extract_component, val_from_js, Effect, PropValue, Component, Val, WidgetCx,
};

// ---------------------------------------------------------------------------
// OpacityComponent — applies an alpha multiplier to its child subtree.
//
// `value` is the opacity in [0.0, 1.0] and is reactive.
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct OpacityComponent {
    pub value: Option<Val<f32>>,
    pub query_key: Option<Vec<String>>,
    pub child: Option<Rc<dyn Component>>,
}

impl Component for OpacityComponent {
    fn build(&self, cx: &mut WidgetCx, boa: &mut Context, parent: ElementNodeId) -> ElementNodeId {
        let id = cx.alloc_node();
        cx.insert_node(id, AnyElement::new(OpacityElement { component: self.clone() }), boa);
        if let Some(qk) = &self.query_key {
            cx.set_query_key(id, qk.clone());
        }
        if let Some(child_spec) = &self.child {
            let _child_id = child_spec.build(cx, boa, id);
        }
        cx.link_child(parent, id);
        id
    }
}

pub struct OpacityElement {
    pub component: OpacityComponent,
}

impl Effect for OpacityElement {}

impl ElementTrace for OpacityElement {
    fn trace_label(&self) -> String {
        if let Some(Val::Static(v)) = &self.component.value {
            format!("opacity={v}")
        } else {
            String::new()
        }
    }
}

impl ElementLayout for OpacityElement {
    fn perform_layout_size(
        &mut self,
        constraints: &Constraints,
        children: &[ElementNodeId],
        cx: &mut LayoutContext,
    ) -> Size {
        if let Some(child_id) = children.first() {
            cx.layout_child(*child_id, constraints)
        } else {
            constraints.constrain(Size::ZERO)
        }
    }

    fn perform_layout_position(&mut self, children: &[ElementNodeId], cx: &mut LayoutContext) {
        if let Some(child_id) = children.first() {
            cx.set_child_offset(*child_id, Offset::ZERO);
        }
    }
}

impl ElementRender for OpacityElement {
    fn type_name(&self) -> &'static str {
        "tur_opacity"
    }

    fn paint(
        &self,
        canvas: &mut dyn Canvas,
        offset: Offset,
        _layout: &ComputedLayout,
        children: &[ElementNodeId],
        paint_ctx: &PaintContext,
    ) {
        let opacity: f32 = paint_ctx
            .read_val_opt(self.component.value.as_ref())
            .unwrap_or(1.0);
        canvas.push_opacity(opacity);
        for &child_id in children {
            paint_ctx.paint_child(child_id, canvas, offset);
        }
        canvas.pop_opacity();
    }
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

fn prop_val<T: PropValue>(props: &JsObject, key: &str, ctx: &mut Context) -> Option<Val<T>> {
    use boa_engine::js_string;
    let v = props.get(js_string!(key), ctx).ok()?;
    val_from_js(&v)
}

fn prop_query_key(props: &JsObject, key: &str, ctx: &mut Context) -> Option<Vec<String>> {
    use boa_engine::object::builtins::JsArray;
    use boa_engine::js_string;
    let v = props.get(js_string!(key), ctx).ok()?;
    let obj = v.as_object()?;
    let arr = JsArray::from_object(obj.clone()).ok()?;
    let len = arr.length(ctx).ok()? as usize;
    let mut out = Vec::with_capacity(len);
    for i in 0..len {
        if let Ok(val) = arr.at(i as i64, ctx) {
            if let Some(s) = val.as_string() {
                out.push(s.to_std_string_escaped());
            }
        }
    }
    if out.is_empty() { None } else { Some(out) }
}

fn prop_child(props: &JsObject, key: &str, ctx: &mut Context) -> Option<Rc<dyn Component>> {
    use boa_engine::js_string;
    let v = props.get(js_string!(key), ctx).ok()?;
    extract_component(&v)
}

impl OpacityComponent {
    pub fn from_js(props: &JsObject, ctx: &mut Context) -> Self {
        OpacityComponent {
            value: prop_val::<f32>(props, "value", ctx),
            query_key: prop_query_key(props, "queryKey", ctx),
            child: prop_child(props, "child", ctx),
        }
    }
}

// ---------------------------------------------------------------------------
// TransformComponent — applies a 2D affine transform to its child subtree.
//
// Supported props: `scale` (uniform), `scaleX`, `scaleY`, `rotate` (radians),
// `translateX`, `translateY`. All reactive.
// ---------------------------------------------------------------------------

#[derive(Clone, Default)]
pub struct TransformComponent {
    pub scale: Option<Val<f64>>,
    pub scale_x: Option<Val<f64>>,
    pub scale_y: Option<Val<f64>>,
    pub rotate: Option<Val<f64>>,
    pub translate_x: Option<Val<f64>>,
    pub translate_y: Option<Val<f64>>,
    pub query_key: Option<Vec<String>>,
    pub child: Option<Rc<dyn Component>>,
}

impl Component for TransformComponent {
    fn build(&self, cx: &mut WidgetCx, boa: &mut Context, parent: ElementNodeId) -> ElementNodeId {
        let id = cx.alloc_node();
        cx.insert_node(id, AnyElement::new(TransformElement { component: self.clone() }), boa);
        if let Some(qk) = &self.query_key {
            cx.set_query_key(id, qk.clone());
        }
        if let Some(child_spec) = &self.child {
            let _child_id = child_spec.build(cx, boa, id);
        }
        cx.link_child(parent, id);
        id
    }
}

pub struct TransformElement {
    pub component: TransformComponent,
}

impl TransformElement {
    /// Resolve the transform during paint using PaintContext (no dep
    /// tracking — paint is a read-only pass).
    fn resolve_transform_paint(&self, paint_ctx: &PaintContext) -> Affine {
        let scale = paint_ctx.read_val_opt(self.component.scale.as_ref());
        let sx = paint_ctx
            .read_val_opt(self.component.scale_x.as_ref())
            .or(scale)
            .unwrap_or(1.0);
        let sy = paint_ctx
            .read_val_opt(self.component.scale_y.as_ref())
            .or(scale)
            .unwrap_or(1.0);
        let angle = paint_ctx
            .read_val_opt(self.component.rotate.as_ref())
            .unwrap_or(0.0);
        let tx = paint_ctx
            .read_val_opt(self.component.translate_x.as_ref())
            .unwrap_or(0.0);
        let ty = paint_ctx
            .read_val_opt(self.component.translate_y.as_ref())
            .unwrap_or(0.0);

        Affine::translate((tx, ty))
            * Affine::rotate(angle)
            * Affine::scale(sx)
            * Affine::scale(sy)
    }
}

impl Effect for TransformElement {}

impl ElementTrace for TransformElement {
    fn trace_label(&self) -> String {
        let mut parts = Vec::new();
        if let Some(Val::Static(v)) = &self.component.scale { parts.push(format!("scale={v}")); }
        if let Some(Val::Static(v)) = &self.component.rotate { parts.push(format!("rotate={v}")); }
        parts.join(" ")
    }
}

impl ElementLayout for TransformElement {
    fn perform_layout_size(
        &mut self,
        constraints: &Constraints,
        children: &[ElementNodeId],
        cx: &mut LayoutContext,
    ) -> Size {
        // Layout the child untransformed; the transform is applied at paint
        // time only. (For non-uniform scale this means layout uses the
        // untransformed size, which is correct for hit-testing but not for
        // visual bounds. Acceptable for animation effects.)
        if let Some(child_id) = children.first() {
            cx.layout_child(*child_id, constraints)
        } else {
            constraints.constrain(Size::ZERO)
        }
    }

    fn perform_layout_position(&mut self, children: &[ElementNodeId], cx: &mut LayoutContext) {
        if let Some(child_id) = children.first() {
            // Touch reactive vals so dep tracking registers this node and
            // the layout re-runs when transform props change.
            let _ = cx.read_val_opt(self.component.scale.as_ref());
            let _ = cx.read_val_opt(self.component.scale_x.as_ref());
            let _ = cx.read_val_opt(self.component.scale_y.as_ref());
            let _ = cx.read_val_opt(self.component.rotate.as_ref());
            let _ = cx.read_val_opt(self.component.translate_x.as_ref());
            let _ = cx.read_val_opt(self.component.translate_y.as_ref());
            cx.set_child_offset(*child_id, Offset::ZERO);
        }
    }
}

impl ElementRender for TransformElement {
    fn type_name(&self) -> &'static str {
        "tur_transform"
    }

    fn paint(
        &self,
        canvas: &mut dyn Canvas,
        offset: Offset,
        _layout: &ComputedLayout,
        children: &[ElementNodeId],
        paint_ctx: &PaintContext,
    ) {
        let local = self.resolve_transform_paint(paint_ctx);
        // Combine the canvas offset (parent-relative origin) with the local
        // transform so the child paints in the right place.
        let combined = Affine::translate((offset.x, offset.y)) * local;
        canvas.push_transform(combined);
        for &child_id in children {
            paint_ctx.paint_child(child_id, canvas, Offset::ZERO);
        }
        canvas.pop_transform();
    }
}

impl TransformComponent {
    pub fn from_js(props: &JsObject, ctx: &mut Context) -> Self {
        TransformComponent {
            scale: prop_val::<f64>(props, "scale", ctx),
            scale_x: prop_val::<f64>(props, "scaleX", ctx),
            scale_y: prop_val::<f64>(props, "scaleY", ctx),
            rotate: prop_val::<f64>(props, "rotate", ctx),
            translate_x: prop_val::<f64>(props, "translateX", ctx),
            translate_y: prop_val::<f64>(props, "translateY", ctx),
            query_key: prop_query_key(props, "queryKey", ctx),
            child: prop_child(props, "child", ctx),
        }
    }
}
