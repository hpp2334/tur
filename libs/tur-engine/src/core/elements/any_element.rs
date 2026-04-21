use boa_engine::{Context, JsString, JsValue};
use tur_shared::{ComputedLayout, Constraints, Offset, Size};

use crate::core::element::{ElementKind, ElementNodeId};
use crate::core::elements::ElementOnUpdate;
use crate::core::layout::{ElementLayout, LayoutContext};
use crate::core::render::{Canvas, ElementRender, PaintContext};

pub struct AnyElement {
    inner: Box<dyn Erased>,
    text_content: Option<String>,
}

trait Erased: Send + Sync + 'static {
    fn kind(&self) -> ElementKind;
    fn type_name(&self) -> &'static str;
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
}

impl<E> Erased for E
where
    E: ElementOnUpdate + ElementLayout + ElementRender + 'static,
{
    fn kind(&self) -> ElementKind {
        ElementKind::new(<Self as ElementRender>::type_name(self))
    }

    fn type_name(&self) -> &'static str {
        <Self as ElementRender>::type_name(self)
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
}

impl AnyElement {
    pub fn new<E: ElementOnUpdate + ElementLayout + ElementRender + 'static>(element: E) -> Self {
        let type_name = <E as ElementRender>::type_name(&element);
        AnyElement {
            inner: Box::new(element),
            text_content: if type_name == "tur_text" {
                Some(String::new())
            } else {
                None
            },
        }
    }

    pub fn kind(&self) -> ElementKind {
        self.inner.kind()
    }

    pub fn type_name(&self) -> &'static str {
        self.inner.type_name()
    }

    pub fn set_prop(&mut self, ctx: &mut Context, key: &JsString, value: &JsValue) {
        if self.text_content.is_some() && *key == "content" {
            if let Some(s) = value.as_string() {
                self.text_content = Some(s.to_std_string_escaped());
            }
        }
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

    pub fn text_content(&self) -> Option<&str> {
        self.text_content.as_deref()
    }
}
