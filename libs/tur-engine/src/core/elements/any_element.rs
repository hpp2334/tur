use std::any::Any;

use boa_engine::object::builtins::JsFunction;
use boa_engine::{Context, JsString, JsValue};
use tur_shared::{ComputedLayout, Constraints, Offset, Size};

use crate::core::element::{ElementKind, ElementNodeId};
use crate::core::js_command::AnyJsCommand;
use crate::core::elements::ElementJsCallbackEmitter;
use crate::core::elements::dispatch_emit_js_callback;
use crate::core::elements::ElementOnUpdate;
use crate::core::elements::ElementTrace;
use crate::core::keyboard::AppKeyEvent;
use crate::core::layout::{ElementLayout, LayoutContext};
use crate::core::render::{Canvas, ElementRender, PaintContext};
use crate::core::elements::{ElementOnIme, ElementOnKeyboard, ElementOnGesture, ElementOnFocus, ComposedGestureEvent, ElementOnGestureContext, ElementOnKeyboardContext, ElementOnImeContext};
use crate::core::event::AppImeEvent;

type KeyboardFn = fn(&mut dyn Any, &mut ElementOnKeyboardContext, &AppKeyEvent);
type GestureFn = fn(&mut dyn Any, &mut ElementOnGestureContext, &ComposedGestureEvent) -> bool;
type ImeFn = fn(&mut dyn Any, &mut ElementOnImeContext, &AppImeEvent);
type EmitJsCallbackFn = fn(&dyn Any, &mut Context, AnyJsCommand) -> Option<(JsFunction, Vec<JsValue>)>;

pub struct AnyElement {
    inner: Box<dyn Erased>,
    on_keyboard: Option<KeyboardFn>,
    on_gesture: Option<GestureFn>,
    on_ime: Option<ImeFn>,
    emit_js_callback_fn: Option<EmitJsCallbackFn>,
}

trait Erased: 'static {
    fn kind(&self) -> ElementKind;
    fn type_name(&self) -> &'static str;
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn trace_label(&self) -> String;
    fn set_prop(&mut self, ctx: &mut Context, key: &JsString, value: &JsValue);
    fn reset_prop(&mut self, key: &JsString);
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
    E: ElementOnUpdate + ElementLayout + ElementRender + ElementTrace + 'static,
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

    fn reset_prop(&mut self, key: &JsString) {
        <Self as ElementOnUpdate>::reset_prop(self, key);
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
}

impl AnyElement {
    pub fn new<E: ElementOnUpdate + ElementLayout + ElementRender + ElementTrace + 'static>(
        element: E,
    ) -> Self {
        AnyElement {
            inner: Box::new(element),
            on_keyboard: None,
            on_gesture: None,
            on_ime: None,
            emit_js_callback_fn: None,
        }
    }

    pub fn with_interactivity<
        E: ElementOnUpdate
            + ElementLayout
            + ElementRender
            + ElementTrace
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
            on_ime: None,
            emit_js_callback_fn: None,
        }
    }

    pub fn with_gesture<
        E: ElementOnUpdate
            + ElementLayout
            + ElementRender
            + ElementTrace
            + ElementOnGesture
            + 'static,
    >(
        element: E,
    ) -> Self {
        AnyElement {
            inner: Box::new(element),
            on_keyboard: None,
            on_gesture: Some(gesture_dispatch::<E>),
            on_ime: None,
            emit_js_callback_fn: None,
        }
    }

    pub fn with_focusability<
        E: ElementOnUpdate
            + ElementLayout
            + ElementRender
            + ElementTrace
            + ElementOnFocus
            + 'static,
    >(
        element: E,
    ) -> Self {
        AnyElement {
            inner: Box::new(element),
            on_keyboard: None,
            on_gesture: None,
            on_ime: None,
            emit_js_callback_fn: None,
        }
    }

    pub fn with_gesture_and_focus<
        E: ElementOnUpdate
            + ElementLayout
            + ElementRender
            + ElementTrace
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
            on_ime: None,
            emit_js_callback_fn: None,
        }
    }

    pub fn with_full_interactivity<
        E: ElementOnUpdate
            + ElementLayout
            + ElementRender
            + ElementTrace
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
            on_ime: Some(ime_dispatch::<E>),
            emit_js_callback_fn: None,
        }
    }

    pub fn with_js_callback_emitter<E: ElementJsCallbackEmitter + 'static>(mut self) -> Self {
        self.emit_js_callback_fn = Some(dispatch_emit_js_callback::<E>);
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

    pub fn set_prop(&mut self, ctx: &mut Context, key: &JsString, value: &JsValue) {
        self.inner.set_prop(ctx, key, value);
    }

    pub fn reset_prop(&mut self, key: &JsString) {
        self.inner.reset_prop(key);
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
        self.emit_js_callback_fn.is_some()
    }

    pub fn emit_js_callback(
        &self,
        context: &mut Context,
        command: AnyJsCommand,
    ) -> Option<(JsFunction, Vec<JsValue>)> {
        let f = self.emit_js_callback_fn?;
        f(self.inner.as_any(), context, command)
    }

    pub fn has_js_callback_emitter(&self) -> bool {
        self.emit_js_callback_fn.is_some()
    }
}
