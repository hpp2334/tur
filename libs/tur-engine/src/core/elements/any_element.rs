use std::any::Any;

use boa_engine::Context;
use tur_shared::{ComputedLayout, Constraints, Offset, Size};

use crate::core::element::{ElementKind, ElementNodeId};
use crate::core::elements::{ElementTrace, TraceValue};
use crate::core::view::Effect;
use crate::core::keyboard::AppKeyEvent;
use crate::core::layout::{ElementLayout, ElementSubscribe, LayoutContext, SubscribeCx};
use crate::core::render::{Canvas, ElementRender, PaintContext};
use crate::core::elements::{ElementOnIme, ElementOnKeyboard, ElementOnGesture, ElementOnFocus, ElementOnWheel, ComposedGestureEvent, ElementOnGestureContext, ElementOnKeyboardContext, ElementOnImeContext, ElementOnWheelContext, WheelEvent};
use crate::core::event::AppImeEvent;

type KeyboardFn = fn(&mut dyn Any, &mut ElementOnKeyboardContext, &AppKeyEvent);
type GestureFn = fn(&mut dyn Any, &mut ElementOnGestureContext, &ComposedGestureEvent);
type WheelFn = fn(&mut dyn Any, &mut ElementOnWheelContext, &WheelEvent) -> f64;
type ImeFn = fn(&mut dyn Any, &mut ElementOnImeContext, &AppImeEvent);

pub struct AnyElement {
    inner: Box<dyn Erased>,
    on_keyboard: Option<KeyboardFn>,
    on_gesture: Option<GestureFn>,
    on_wheel: Option<WheelFn>,
    on_ime: Option<ImeFn>,
    has_callbacks: bool,
}

trait Erased: 'static {
    fn kind(&self) -> ElementKind;
    fn type_name(&self) -> &'static str;
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn trace_label(&self) -> String;
    fn trace_props(&self) -> Vec<(&'static str, TraceValue)>;
    fn trace_layout_extra(&self) -> Vec<(&'static str, TraceValue)>;

    fn perform_layout(
        &mut self,
        constraints: &Constraints,
        children: &[ElementNodeId],
        cx: &mut LayoutContext,
    ) -> Size;
    fn paint(
        &self,
        canvas: &mut dyn Canvas,
        offset: Offset,
        layout: &ComputedLayout,
        children: &[ElementNodeId],
        paint_ctx: &PaintContext,
    );
    fn hit_test(&self, position: Offset, layout: &ComputedLayout) -> bool;

    fn subscribe(&self, cx: &mut SubscribeCx);

    fn run_effect(
        &mut self,
        cx: &mut crate::core::view::SharedViewCx,
        boa: &mut Context,
        dirties: &std::collections::HashSet<crate::core::reactive::AtomId>,
    );
}

fn keyboard_dispatch<E: ElementOnKeyboard + 'static>(
    any: &mut dyn Any,
    cx: &mut ElementOnKeyboardContext,
    event: &AppKeyEvent,
) {
    let element = any.downcast_mut::<E>().unwrap();
    ElementOnKeyboard::on_keyboard_event(element, cx, event);
}

fn gesture_dispatch<E: ElementOnGesture + 'static>(
    any: &mut dyn Any,
    cx: &mut ElementOnGestureContext,
    event: &ComposedGestureEvent,
) {
    let element = any.downcast_mut::<E>().unwrap();
    ElementOnGesture::on_gesture_event(element, cx, event);
}

fn wheel_dispatch<E: ElementOnWheel + 'static>(
    any: &mut dyn Any,
    cx: &mut ElementOnWheelContext,
    event: &WheelEvent,
) -> f64 {
    let element = any.downcast_mut::<E>().unwrap();
    ElementOnWheel::on_wheel(element, cx, event)
}

fn ime_dispatch<E: ElementOnIme + 'static>(
    any: &mut dyn Any,
    cx: &mut ElementOnImeContext,
    event: &AppImeEvent,
) {
    let element = any.downcast_mut::<E>().unwrap();
    ElementOnIme::on_ime_event(element, cx, event);
}

impl<E> Erased for E
where
    E: ElementLayout + ElementRender + ElementTrace + Effect + ElementSubscribe + 'static,
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

    fn trace_props(&self) -> Vec<(&'static str, TraceValue)> {
        <Self as ElementTrace>::trace_props(self)
    }

    fn trace_layout_extra(&self) -> Vec<(&'static str, TraceValue)> {
        <Self as ElementTrace>::trace_layout_extra(self)
    }

    fn perform_layout(
        &mut self,
        constraints: &Constraints,
        children: &[ElementNodeId],
        cx: &mut LayoutContext,
    ) -> Size {
        <Self as ElementLayout>::perform_layout(self, constraints, children, cx)
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

    fn subscribe(&self, cx: &mut SubscribeCx) {
        <Self as ElementSubscribe>::subscribe(self, cx)
    }

    fn run_effect(
        &mut self,
        cx: &mut crate::core::view::SharedViewCx,
        boa: &mut Context,
        dirties: &std::collections::HashSet<crate::core::reactive::AtomId>,
    ) {
        <Self as Effect>::effect(self, cx, boa, dirties);
    }
}

impl AnyElement {
    pub fn new<E: ElementLayout + ElementRender + ElementTrace + Effect + ElementSubscribe + 'static>(
        element: E,
    ) -> Self {
        AnyElement {
            inner: Box::new(element),
            on_keyboard: None,
            on_gesture: None,
            on_wheel: None,
            on_ime: None,
            has_callbacks: false,
        }
    }

    pub fn with_interactivity<
        E: ElementLayout
            + ElementRender
            + ElementTrace
            + Effect
            + ElementSubscribe
            + ElementOnKeyboard
            + ElementOnGesture
            + 'static,
    >(
        element: E,
    ) -> Self {
        AnyElement {
            inner: Box::new(element),
            on_keyboard: Some(keyboard_dispatch::<E>),
            on_gesture: Some(gesture_dispatch::<E>),
            on_wheel: None,
            on_ime: None,
            has_callbacks: false,
        }
    }

    pub fn with_gesture<
        E: ElementLayout
            + ElementRender
            + ElementTrace
            + Effect
            + ElementSubscribe
            + ElementOnGesture
            + 'static,
    >(
        element: E,
    ) -> Self {
        AnyElement {
            inner: Box::new(element),
            on_keyboard: None,
            on_gesture: Some(gesture_dispatch::<E>),
            on_wheel: None,
            on_ime: None,
            has_callbacks: false,
        }
    }

    pub fn with_wheel<
        E: ElementLayout
            + ElementRender
            + ElementTrace
            + Effect
            + ElementSubscribe
            + ElementOnWheel
            + 'static,
    >(
        element: E,
    ) -> Self {
        AnyElement {
            inner: Box::new(element),
            on_keyboard: None,
            on_gesture: None,
            on_wheel: Some(wheel_dispatch::<E>),
            on_ime: None,
            has_callbacks: false,
        }
    }

    pub fn with_focusability<
        E: ElementLayout
            + ElementRender
            + ElementTrace
            + Effect
            + ElementSubscribe
            + ElementOnFocus
            + 'static,
    >(
        element: E,
    ) -> Self {
        AnyElement {
            inner: Box::new(element),
            on_keyboard: None,
            on_gesture: None,
            on_wheel: None,
            on_ime: None,
            has_callbacks: false,
        }
    }

    pub fn with_gesture_and_focus<
        E: ElementLayout
            + ElementRender
            + ElementTrace
            + Effect
            + ElementSubscribe
            + ElementOnFocus
            + ElementOnGesture
            + 'static,
    >(
        element: E,
    ) -> Self {
        AnyElement {
            inner: Box::new(element),
            on_keyboard: None,
            on_gesture: Some(gesture_dispatch::<E>),
            on_wheel: None,
            on_ime: None,
            has_callbacks: false,
        }
    }

    pub fn with_full_interactivity<
        E: ElementLayout
            + ElementRender
            + ElementTrace
            + Effect
            + ElementSubscribe
            + ElementOnKeyboard
            + ElementOnGesture
            + ElementOnFocus
            + ElementOnIme
            + 'static,
    >(
        element: E,
    ) -> Self {
        AnyElement {
            inner: Box::new(element),
            on_keyboard: Some(keyboard_dispatch::<E>),
            on_gesture: Some(gesture_dispatch::<E>),
            on_wheel: None,
            on_ime: Some(ime_dispatch::<E>),
            has_callbacks: false,
        }
    }

    pub fn with_callbacks(mut self) -> Self {
        self.has_callbacks = true;
        self
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

    pub fn trace_props(&self) -> Vec<(&'static str, TraceValue)> {
        self.inner.trace_props()
    }

    pub fn trace_layout_extra(&self) -> Vec<(&'static str, TraceValue)> {
        self.inner.trace_layout_extra()
    }

    pub fn perform_layout(
        &mut self,
        constraints: &Constraints,
        children: &[ElementNodeId],
        cx: &mut LayoutContext,
    ) -> Size {
        self.inner.perform_layout(constraints, children, cx)
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

    /// Declare this element's reactive atom dependencies into `cx` (the
    /// explicit subscribe phase). No-op for elements without reactive props.
    pub fn subscribe(&self, cx: &mut SubscribeCx) {
        self.inner.subscribe(cx)
    }

    /// Run the view's effect hook (Condition branch swap, LazyList range
    /// adjustment, etc.). No-op for most views.
    pub fn run_effect(
        &mut self,
        cx: &mut crate::core::view::SharedViewCx,
        boa: &mut Context,
        dirties: &std::collections::HashSet<crate::core::reactive::AtomId>,
    ) {
        self.inner.run_effect(cx, boa, dirties);
    }

    pub fn on_keyboard_event(
        &mut self,
        cx: &mut ElementOnKeyboardContext,
        event: &AppKeyEvent,
    ) {
        if let Some(handler) = self.on_keyboard {
            handler(self.inner.as_any_mut(), cx, event);
        }
    }

    pub fn on_gesture_event(
        &mut self,
        cx: &mut ElementOnGestureContext,
        event: &ComposedGestureEvent,
    ) {
        if let Some(handler) = self.on_gesture {
            handler(self.inner.as_any_mut(), cx, event);
        }
    }

    pub fn on_wheel_event(
        &mut self,
        cx: &mut ElementOnWheelContext,
        event: &WheelEvent,
    ) -> f64 {
        if let Some(handler) = self.on_wheel {
            handler(self.inner.as_any_mut(), cx, event)
        } else {
            0.0
        }
    }

    pub fn has_on_wheel(&self) -> bool {
        self.on_wheel.is_some()
    }

    pub fn on_ime_event(
        &mut self,
        cx: &mut ElementOnImeContext,
        event: &AppImeEvent,
    ) {
        if let Some(handler) = self.on_ime {
            handler(self.inner.as_any_mut(), cx, event);
        }
    }

    pub fn has_focus(&self) -> bool {
        self.has_callbacks
    }
}
