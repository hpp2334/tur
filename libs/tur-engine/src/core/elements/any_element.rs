use std::any::Any;

use boa_engine::{Context, JsString, JsValue};
use tur_shared::{ComputedLayout, Constraints, Offset, Size};

use crate::core::element::{ElementKind, ElementNodeId};
use crate::core::elements::{ElementOnGesture, ElementOnKeyboard, KeyboardResult};
use crate::core::elements::ElementOnUpdate;
use crate::core::elements::ElementTrace;
use crate::core::elements::{ComposedGestureEvent, ElementOnGestureContext, GestureChanges, GestureResult};
use crate::core::keyboard::AppKeyEvent;
use crate::core::layout::{ElementLayout, LayoutContext};
use crate::core::render::{Canvas, ElementRender, PaintContext};

pub struct AnyElement {
    inner: Box<dyn Erased>,
}

trait Erased: 'static {
    fn kind(&self) -> ElementKind;
    fn type_name(&self) -> &'static str;
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn trace_label(&self) -> String;
    fn set_prop(&mut self, ctx: &mut Context, key: &JsString, value: &JsValue);
    fn perform_layout_size(
        &mut self,
        constraints: &Constraints,
        children: &[ElementNodeId],
        cx: &mut LayoutContext,
    ) -> Size;
    fn perform_layout_position(&mut self, children: &[ElementNodeId], cx: &mut LayoutContext);
    fn paint(
        &self,
        canvas: &mut dyn Canvas,
        offset: Offset,
        layout: &ComputedLayout,
        children: &[ElementNodeId],
        paint_ctx: &PaintContext,
    );
    fn hit_test(&self, position: Offset, layout: &ComputedLayout) -> bool;
    fn on_keyboard_event(&mut self, event: &AppKeyEvent) -> KeyboardResult;
    fn on_gesture_event(
        &mut self,
        event: &ComposedGestureEvent,
        cx: &mut ElementOnGestureContext,
    ) -> GestureResult;
    fn drain_changes(&mut self) -> GestureChanges;
}

impl<E> Erased for E
where
    E: ElementOnUpdate
        + ElementLayout
        + ElementRender
        + ElementTrace
        + ElementOnKeyboard
        + ElementOnGesture
        + 'static,
{
    fn kind(&self) -> ElementKind {
        ElementKind::new(<Self as ElementRender>::type_name(self))
    }

    fn type_name(&self) -> &'static str {
        <Self as ElementRender>::type_name(self)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn trace_label(&self) -> String {
        let label = <Self as ElementTrace>::trace_label(self);
        if label.is_empty() {
            format!("[{}]", <Self as ElementRender>::type_name(self))
        } else {
            label
        }
    }

    fn set_prop(&mut self, ctx: &mut Context, key: &JsString, value: &JsValue) {
        <Self as ElementOnUpdate>::set_prop(self, ctx, key, value);
    }

    fn perform_layout_size(
        &mut self,
        constraints: &Constraints,
        children: &[ElementNodeId],
        cx: &mut LayoutContext,
    ) -> Size {
        <Self as ElementLayout>::perform_layout_size(self, constraints, children, cx)
    }

    fn perform_layout_position(&mut self, children: &[ElementNodeId], cx: &mut LayoutContext) {
        <Self as ElementLayout>::perform_layout_position(self, children, cx);
    }

    fn paint(
        &self,
        canvas: &mut dyn Canvas,
        offset: Offset,
        layout: &ComputedLayout,
        children: &[ElementNodeId],
        paint_ctx: &PaintContext,
    ) {
        <Self as ElementRender>::paint(self, canvas, offset, layout, children, paint_ctx);
    }

    fn hit_test(&self, position: Offset, layout: &ComputedLayout) -> bool {
        <Self as ElementRender>::hit_test(self, position, layout)
    }

    fn on_keyboard_event(&mut self, event: &AppKeyEvent) -> KeyboardResult {
        <Self as ElementOnKeyboard>::on_keyboard_event(self, event)
    }

    fn on_gesture_event(
        &mut self,
        event: &ComposedGestureEvent,
        cx: &mut ElementOnGestureContext,
    ) -> GestureResult {
        <Self as ElementOnGesture>::on_gesture_event(self, event, cx)
    }

    fn drain_changes(&mut self) -> GestureChanges {
        <Self as ElementOnGesture>::drain_changes(self)
    }
}

impl AnyElement {
    pub fn new<
        E: ElementOnUpdate
            + ElementLayout
            + ElementRender
            + ElementTrace
            + ElementOnKeyboard
            + ElementOnGesture
            + 'static,
    >(element: E) -> Self {
        AnyElement {
            inner: Box::new(element),
        }
    }

    pub fn kind(&self) -> ElementKind {
        self.inner.kind()
    }

    pub fn type_name(&self) -> &'static str {
        self.inner.type_name()
    }

    pub fn cast<T: 'static>(&self) -> Option<&T> {
        self.inner.as_any().downcast_ref::<T>()
    }

    pub fn cast_mut<T: 'static>(&mut self) -> Option<&mut T> {
        self.inner.as_any_mut().downcast_mut::<T>()
    }

    pub fn trace_label(&self) -> String {
        self.inner.trace_label()
    }

    pub fn set_prop(&mut self, ctx: &mut Context, key: &JsString, value: &JsValue) {
        self.inner.set_prop(ctx, key, value);
    }

    pub fn perform_layout_size(
        &mut self,
        constraints: &Constraints,
        children: &[ElementNodeId],
        cx: &mut LayoutContext,
    ) -> Size {
        self.inner.perform_layout_size(constraints, children, cx)
    }

    pub fn perform_layout_position(&mut self, children: &[ElementNodeId], cx: &mut LayoutContext) {
        self.inner.perform_layout_position(children, cx);
    }

    pub fn paint(
        &self,
        canvas: &mut dyn Canvas,
        offset: Offset,
        layout: &ComputedLayout,
        children: &[ElementNodeId],
        paint_ctx: &PaintContext,
    ) {
        self.inner
            .paint(canvas, offset, layout, children, paint_ctx);
    }

    pub fn hit_test(&self, position: Offset, layout: &ComputedLayout) -> bool {
        self.inner.hit_test(position, layout)
    }

    pub fn on_keyboard_event(&mut self, event: &AppKeyEvent) -> KeyboardResult {
        self.inner.on_keyboard_event(event)
    }

    pub fn on_gesture_event(
        &mut self,
        event: &ComposedGestureEvent,
        cx: &mut ElementOnGestureContext,
    ) -> GestureResult {
        self.inner.on_gesture_event(event, cx)
    }

    pub fn drain_changes(&mut self) -> GestureChanges {
        self.inner.drain_changes()
    }
}
