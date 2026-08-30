//! `VirtualAppView` — the element that hosts a virtual app (a complete
//! nested engine instance: own worker, realm, store, tree) and draws the
//! child's latest frame **from its own paint**, replaying the child's
//! `RenderCommandBatch` through ordinary canvas ops (zero core render-model
//! changes — the child's content is just element-drawn content).

use std::cell::Cell;
use std::rc::Rc;

use boa_engine::Context;
use boa_engine::object::JsObject;

use crate::core::element::{ElementNodeId, NodeId};
use crate::core::elements::{AnyElement, ElementTrace, TraceValue};
use crate::core::js_runtime::JsProps;
use crate::core::layout::{Constraints, Geometry, Offset, Size};
use crate::core::layout::{ElementLayout, ElementSubscribe, LayoutContext, SubscribeCx};
use crate::core::render::brush::{Brush, Color};
use crate::core::render::{Canvas, CanvasOp, ElementRender, PaintContext, RenderCommand};
use crate::core::view::{Lifecycle, Val, View, ViewCx};

use super::state::{VirtualControllerRef, VirtualState};

// ---------------------------------------------------------------------------
// VirtualAppView — the user's declaration. Pure Rust, no JsValues.
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct VirtualAppView {
    pub(crate) state: Rc<VirtualState>,
    /// Reactive controller binding (`Readable<VirtualAppController | null>`
    /// on the JS side) — resolved untracked during layout like every other
    /// `Val<T>` prop. Binding materializes the controller's child (lazy
    /// declaration); unbinding (null / swap) destroys it unless `keepAlive`.
    pub(crate) app: Option<Val<VirtualControllerRef>>,
    pub(crate) background: Option<Val<Color>>,
    pub(crate) width: Option<Val<f64>>,
    pub(crate) height: Option<Val<f64>>,
    pub(crate) query_key: Option<Vec<String>>,
    /// Painted while the child isn't live (JS-side `fallback` view).
    pub(crate) fallback: Option<Rc<dyn View>>,
    /// Painted on error (JS-side `errorView`).
    pub(crate) error_view: Option<Rc<dyn View>>,
}

impl View for VirtualAppView {
    fn build(&self, cx: &mut dyn ViewCx, boa: &mut Context, parent: NodeId) -> NodeId {
        let id: ElementNodeId = ElementNodeId::new(cx.alloc_node().as_u64());
        cx.insert_node(
            id,
            AnyElement::new(VirtualAppElement {
                view: self.clone(),
                painting: VirtualPainting::default(),
                bound_base: Cell::new(None),
            }),
            boa,
        );
        if let Some(qk) = &self.query_key {
            cx.set_query_key(id, qk.clone());
        }
        if let Some(child) = &self.fallback {
            let _ = child.build(cx, boa, id.into());
        }
        if let Some(child) = &self.error_view {
            let _ = child.build(cx, boa, id.into());
        }
        cx.link_child(parent, id.into());
        id.into()
    }
}

impl VirtualAppView {
    /// Build a `VirtualAppView` from a JS props object.
    pub fn from_js(props: &JsObject, ctx: &mut Context, state: Rc<VirtualState>) -> Self {
        let mut p = JsProps::new(props, ctx);
        VirtualAppView {
            state,
            app: p.val::<VirtualControllerRef>("app$"),
            background: p.val::<Color>("background"),
            width: p.val::<f64>("width"),
            height: p.val::<f64>("height"),
            query_key: p.query_key("queryKey"),
            fallback: p.child("fallback"),
            error_view: p.child("errorView"),
        }
    }
}

// ---------------------------------------------------------------------------
// VirtualAppElement — the built element.
// ---------------------------------------------------------------------------

#[derive(Default, Clone)]
pub(crate) struct VirtualPainting {
    pub(crate) app: Option<VirtualControllerRef>,
    pub(crate) background: Option<Color>,
}

pub struct VirtualAppElement {
    pub(crate) view: VirtualAppView,
    pub(crate) painting: VirtualPainting,
    /// The controller base this element currently binds (layout-time
    /// bind/unbind diff — see `perform_layout`).
    pub(crate) bound_base: Cell<Option<u64>>,
}

impl Lifecycle for VirtualAppElement {
    fn before_destroy(&mut self, _cx: &mut crate::core::view::SharedViewCx, _boa: &mut Context) {
        if let Some(base) = self.bound_base.take() {
            self.view.state.unbind(base);
        }
    }
}

impl ElementLayout for VirtualAppElement {
    fn perform_layout(
        &mut self,
        constraints: &Constraints,
        _children: &[ElementNodeId],
        cx: &mut LayoutContext,
    ) -> Size {
        let app = cx.read_val_opt(self.view.app.as_ref());
        let background = cx.read_val_opt(self.view.background.as_ref());
        let width = cx.read_val_opt(self.view.width.as_ref());
        let height = cx.read_val_opt(self.view.height.as_ref());

        // Resolve paint props here (layout holds the store); paint reads
        // `self.painting` and never touches the store.
        self.painting = VirtualPainting {
            app: app.clone(),
            background,
        };

        // Bind/unbind diff — the controller is a lazy declaration,
        // materialized by binding (same "pure declaration, materialize on
        // demand" shape as `source`/`derive`/`mutate`). Runs in layout so a
        // resolution change can never skip it: `app$` is subscribed below.
        let new_base = app.as_ref().map(|r| r.0);
        if new_base != self.bound_base.get() {
            if let Some(old) = self.bound_base.take() {
                self.view.state.unbind(old);
            }
            if let Some(base) = new_base {
                self.view.state.bind(base);
            }
            self.bound_base.set(new_base);
        }

        // Leaf-box sizing: explicit width/height or fill the constraints
        // (an `Expanded` parent gives tight bounds; the root fills the
        // viewport).
        let w = width.unwrap_or_else(|| {
            if constraints.max_width.is_finite() {
                constraints.max_width
            } else {
                0.0
            }
        });
        let h = height.unwrap_or_else(|| {
            if constraints.max_height.is_finite() {
                constraints.max_height
            } else {
                0.0
            }
        });
        constraints.constrain(Size::new(w, h))
    }
}

impl ElementSubscribe for VirtualAppElement {
    fn subscribe(&self, cx: &mut SubscribeCx) {
        if let Some(v) = self.view.app.as_ref() {
            cx.subscribe_val(v);
        }
        if let Some(v) = self.view.background.as_ref() {
            cx.subscribe_val(v);
        }
        if let Some(v) = self.view.width.as_ref() {
            cx.subscribe_val(v);
        }
        if let Some(v) = self.view.height.as_ref() {
            cx.subscribe_val(v);
        }
        // Re-layout when the bound controller's status flips — paint reads
        // it to decide fallback vs child frame.
        if let Some(app) = &self.painting.app
            && let Some(record) = self.view.state.record(app.0)
        {
            cx.subscribe_readable(crate::core::edgy::reactive::Readable::from(record.status));
        }
    }
}

impl ElementTrace for VirtualAppElement {
    fn trace_label(&self) -> String {
        match self.painting.app.as_ref() {
            Some(app) => format!("virtualApp#{}", app.0),
            None => "virtualApp idle".to_string(),
        }
    }

    fn trace_props(&self) -> Vec<(&'static str, TraceValue)> {
        let mut p = Vec::new();
        if let Some(app) = self.painting.app.as_ref() {
            p.push(("app", TraceValue::Num(app.0 as f64)));
        }
        p
    }
}

impl ElementRender for VirtualAppElement {
    fn type_name(&self) -> &'static str {
        "tur_virtual_app"
    }

    fn paint(
        &self,
        canvas: &mut dyn Canvas,
        layout: &crate::core::layout::ComputedLayout,
        children: &[ElementNodeId],
        paint_ctx: &PaintContext,
    ) {
        // Clip everything (background, fallback, child frame) to the
        // element rect — existing layer machinery.
        canvas.push_clip(Offset::ZERO, layout.size);
        if let Some(color) = self.painting.background {
            canvas.fill_geometry(
                Offset::ZERO,
                &Geometry::Rect(layout.size),
                &Brush::SolidColor(color),
            );
        }
        for &child_id in children {
            paint_ctx.paint_child(child_id, canvas);
        }
        // Replay the child's latest frame, if one is live.
        if let Some(app) = &self.painting.app
            && let Some(record) = self.view.state.record(app.0)
            && let Some(token) = record.current.get()
            && let Some(batch) = self.view.state.output(token)
        {
            let remap = self.view.state.image_remap(token);
            replay_batch(canvas, &batch, &remap);
        }
        canvas.pop_clip();
    }
}

/// Replay a child's `RenderCommandBatch` into the current canvas context.
///
/// The op executes under the element's absolute transform (applied by the
/// ordinary parent pipeline), which *is* the correct base for the child:
/// child-viewport coordinates map 1:1 onto element-local coordinates (the
/// child's viewport origin is the element's local origin). Per command:
/// push the child node's absolute transform, emit its ops (image ids
/// re-keyed through `remap`), pop. Cross-command layer state (a flex's
/// `PushClip` spanning its children) is preserved — the replay interleaves
/// neutrally, exactly like native linear playback.
fn replay_batch(
    canvas: &mut dyn Canvas,
    batch: &[RenderCommand],
    remap: &std::collections::HashMap<
        crate::core::image_resource::ImageResourceId,
        crate::core::image_resource::ImageResourceId,
    >,
) {
    for command in batch {
        let RenderCommand::Paint { transform, ops, .. } = command;
        canvas.push_transform(*transform);
        for op in ops {
            match op {
                CanvasOp::DrawImage {
                    resource_id,
                    natural_size,
                    transform,
                } => {
                    let id = remap.get(resource_id).copied().unwrap_or(*resource_id);
                    canvas.draw_image(id, *natural_size, *transform);
                }
                CanvasOp::FillGeometry {
                    offset,
                    geometry,
                    brush,
                } => canvas.fill_geometry(*offset, geometry, brush),
                CanvasOp::StrokeGeometry {
                    offset,
                    geometry,
                    color,
                    stroke_width,
                } => canvas.stroke_geometry(*offset, geometry, color, *stroke_width),
                CanvasOp::FillTextLayout { offset, layout } => {
                    canvas.fill_text_layout(*offset, layout)
                }
                CanvasOp::DrawShadow {
                    offset,
                    size,
                    color,
                    border_radius,
                    blur,
                    shadow_offset,
                } => {
                    canvas.draw_shadow(*offset, *size, color, *border_radius, *blur, *shadow_offset)
                }
                CanvasOp::PushClip { offset, size } => canvas.push_clip(*offset, *size),
                CanvasOp::PushClipGeometry { offset, geometry } => {
                    canvas.push_clip_geometry(*offset, geometry)
                }
                CanvasOp::PopClip => canvas.pop_clip(),
                CanvasOp::PushOpacity(o) => canvas.push_opacity(*o),
                CanvasOp::PopOpacity => canvas.pop_opacity(),
                CanvasOp::PushTransform(t) => canvas.push_transform(*t),
                CanvasOp::PopTransform => canvas.pop_transform(),
            }
        }
        canvas.pop_transform();
    }
}
