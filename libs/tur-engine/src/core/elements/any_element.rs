use std::any::Any;

use boa_engine::Context;
use tur_shared::{ComputedLayout, Constraints, Offset, Size};

use crate::core::element::{ElementKind, ElementNodeId};
use crate::core::elements::{ElementTrace, TraceValue};
use crate::core::view::Lifecycle;
use crate::core::keyboard::AppKeyEvent;
use crate::core::layout::{ElementLayout, ElementSubscribe, LayoutContext, SubscribeCx};
use crate::core::render::{Canvas, ElementRender, PaintContext};
use crate::core::elements::{ElementOnIme, ElementOnKeyboard, ElementOnGesture, ElementOnFocus, ElementOnWheel, ComposedGestureEvent, ElementOnGestureContext, ElementOnKeyboardContext, ElementOnImeContext, ElementOnWheelContext, WheelEvent};
use crate::core::event::AppImeEvent;
use crate::core::focus::Focusable;

type KeyboardFn = fn(&mut dyn Any, &mut ElementOnKeyboardContext, &AppKeyEvent);
type GestureFn = fn(&mut dyn Any, &mut ElementOnGestureContext, &ComposedGestureEvent) -> bool;
type WheelFn = fn(&mut dyn Any, &mut ElementOnWheelContext, &WheelEvent) -> f64;
type ImeFn = fn(&mut dyn Any, &mut ElementOnImeContext, &AppImeEvent);
type CursorRectFn = fn(&dyn Any) -> Option<(f64, f64, f64, f64)>;
type FocusableCastFn = fn(&dyn Any) -> Option<&dyn Focusable>;

pub struct AnyElement {
    inner: Box<dyn Erased>,
    on_keyboard: Option<KeyboardFn>,
    on_gesture: Option<GestureFn>,
    on_wheel: Option<WheelFn>,
    on_ime: Option<ImeFn>,
    cursor_rect_fn: Option<CursorRectFn>,
    focusable_fn: Option<FocusableCastFn>,
    has_callbacks: bool,
}

pub trait ElementCursorRect {
    fn cursor_rect_relative(&self) -> Option<(f64, f64, f64, f64)>;
}

fn cursor_rect_dispatch<E: ElementCursorRect + 'static>(
    any: &dyn Any,
) -> Option<(f64, f64, f64, f64)> {
    any.downcast_ref::<E>().and_then(|e: &E| e.cursor_rect_relative())
}

fn focus_cast_dispatch<E: Focusable + 'static>(any: &dyn Any) -> Option<&dyn Focusable> {
    any.downcast_ref::<E>().map(|e: &E| e as &dyn Focusable)
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

    fn run_on_mounted(
        &mut self,
        cx: &mut crate::core::view::SharedViewCx,
        boa: &mut Context,
    );
    fn run_on_updated(
        &mut self,
        cx: &mut crate::core::view::SharedViewCx,
        boa: &mut Context,
    );
    fn run_before_destroy(
        &mut self,
        cx: &mut crate::core::view::SharedViewCx,
        boa: &mut Context,
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
) -> bool {
    let element = any.downcast_mut::<E>().unwrap();
    ElementOnGesture::on_gesture_event(element, cx, event)
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
    E: ElementLayout + ElementRender + ElementTrace + Lifecycle + ElementSubscribe + 'static,
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

    fn run_on_mounted(
        &mut self,
        cx: &mut crate::core::view::SharedViewCx,
        boa: &mut Context,
    ) {
        <Self as Lifecycle>::on_mounted(self, cx, boa);
    }

    fn run_on_updated(
        &mut self,
        cx: &mut crate::core::view::SharedViewCx,
        boa: &mut Context,
    ) {
        <Self as Lifecycle>::on_updated(self, cx, boa);
    }

    fn run_before_destroy(
        &mut self,
        cx: &mut crate::core::view::SharedViewCx,
        boa: &mut Context,
    ) {
        <Self as Lifecycle>::before_destroy(self, cx, boa);
    }
}

impl AnyElement {
    pub fn new<E: ElementLayout + ElementRender + ElementTrace + Lifecycle + ElementSubscribe + 'static>(
        element: E,
    ) -> Self {
        AnyElement {
            inner: Box::new(element),
            on_keyboard: None,
            on_gesture: None,
            on_wheel: None,
            on_ime: None,
            cursor_rect_fn: None,
            focusable_fn: None,
            has_callbacks: false,
        }
    }

    pub fn with_interactivity<
        E: ElementLayout
            + ElementRender
            + ElementTrace
            + Lifecycle
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
            cursor_rect_fn: None,
            focusable_fn: None,
            has_callbacks: false,
        }
    }

    pub fn with_gesture<
        E: ElementLayout
            + ElementRender
            + ElementTrace
            + Lifecycle
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
            cursor_rect_fn: None,
            focusable_fn: None,
            has_callbacks: false,
        }
    }

    pub fn with_wheel<
        E: ElementLayout
            + ElementRender
            + ElementTrace
            + Lifecycle
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
            cursor_rect_fn: None,
            focusable_fn: None,
            has_callbacks: false,
        }
    }

    pub fn with_focusability<
        E: ElementLayout
            + ElementRender
            + ElementTrace
            + Lifecycle
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
            cursor_rect_fn: None,
            focusable_fn: None,
            has_callbacks: false,
        }
    }

    pub fn with_gesture_and_focus<
        E: ElementLayout
            + ElementRender
            + ElementTrace
            + Lifecycle
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
            cursor_rect_fn: None,
            focusable_fn: None,
            has_callbacks: false,
        }
    }

    pub fn with_full_interactivity<
        E: ElementLayout
            + ElementRender
            + ElementTrace
            + Lifecycle
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
            cursor_rect_fn: None,
            focusable_fn: None,
            has_callbacks: false,
        }
    }

    pub fn with_callbacks(mut self) -> Self {
        self.has_callbacks = true;
        self
    }

    pub fn with_cursor_rect<E: ElementCursorRect + 'static>(mut self) -> Self {
        self.cursor_rect_fn = Some(cursor_rect_dispatch::<E>);
        self
    }

    pub fn with_focusable<E: Focusable + 'static>(mut self) -> Self {
        self.focusable_fn = Some(focus_cast_dispatch::<E>);
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

    /// Fire the element's `on_mounted` lifecycle hook (called once, right
    /// after the element is inserted into the tree). No-op for most elements.
    pub fn run_on_mounted(
        &mut self,
        cx: &mut crate::core::view::SharedViewCx,
        boa: &mut Context,
    ) {
        self.inner.run_on_mounted(cx, boa);
    }

    /// Fire the element's `on_updated` lifecycle hook (called after layout,
    /// for elements whose subscribed atoms were dirtied this flush).
    /// No-op for most elements.
    pub fn run_on_updated(
        &mut self,
        cx: &mut crate::core::view::SharedViewCx,
        boa: &mut Context,
    ) {
        self.inner.run_on_updated(cx, boa);
    }

    /// Fire the element's `before_destroy` lifecycle hook (called once,
    /// immediately before the element is removed from the tree).
    /// No-op for most elements.
    pub fn run_before_destroy(
        &mut self,
        cx: &mut crate::core::view::SharedViewCx,
        boa: &mut Context,
    ) {
        self.inner.run_before_destroy(cx, boa);
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
    ) -> bool {
        if let Some(handler) = self.on_gesture {
            handler(self.inner.as_any_mut(), cx, event)
        } else {
            false
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

    pub fn has_on_gesture(&self) -> bool {
        self.on_gesture.is_some()
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

    pub fn cursor_rect_relative(&self) -> Option<(f64, f64, f64, f64)> {
        let any = self.inner.as_any();
        self.cursor_rect_fn.and_then(|f| f(any))
    }

    pub fn as_focusable(&self) -> Option<&dyn Focusable> {
        let any = self.inner.as_any();
        self.focusable_fn.and_then(|f| f(any))
    }
}
