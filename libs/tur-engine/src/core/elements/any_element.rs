use boa_engine::{Context, JsString, JsValue};
use tur_shared::{ComputedLayout, Constraints, Offset, Size};

use crate::core::render::{ChildLayout, ChildPaint, PaintContext};
use crate::core::traits::{
    ElementKind, ElementLayout, ElementNodeId, ElementOnUpdate, ElementRender,
};

pub struct AnyElement {
    inner: Box<dyn Erased>,
}

trait Erased: Send + Sync + 'static {
    fn kind(&self) -> ElementKind;
    fn type_name(&self) -> &'static str;
    fn set_prop(&mut self, ctx: &mut Context, key: &JsString, value: &JsValue);
    fn perform_layout_size(
        &mut self,
        constraints: &Constraints,
        children: &[ElementNodeId],
        child_layout: &mut dyn ChildLayout,
    ) -> Size;
    fn perform_layout_position(
        &mut self,
        children: &[ElementNodeId],
        child_layout: &mut dyn ChildLayout,
    );
    fn paint(
        &self,
        ctx: &mut dyn PaintContext,
        offset: Offset,
        layout: &ComputedLayout,
        children: &[ElementNodeId],
        child_paint: &mut dyn ChildPaint,
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
        child_layout: &mut dyn ChildLayout,
    ) -> Size {
        <Self as ElementLayout>::perform_layout_size(self, constraints, children, child_layout)
    }

    fn perform_layout_position(
        &mut self,
        children: &[ElementNodeId],
        child_layout: &mut dyn ChildLayout,
    ) {
        <Self as ElementLayout>::perform_layout_position(self, children, child_layout);
    }

    fn paint(
        &self,
        ctx: &mut dyn PaintContext,
        offset: Offset,
        layout: &ComputedLayout,
        children: &[ElementNodeId],
        child_paint: &mut dyn ChildPaint,
    ) {
        <Self as ElementRender>::paint(self, ctx, offset, layout, children, child_paint);
    }

    fn hit_test(&self, position: Offset, layout: &ComputedLayout) -> bool {
        <Self as ElementRender>::hit_test(self, position, layout)
    }
}

impl AnyElement {
    pub fn new<E: ElementOnUpdate + ElementLayout + ElementRender + 'static>(element: E) -> Self {
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

    pub fn set_prop(&mut self, ctx: &mut Context, key: &JsString, value: &JsValue) {
        self.inner.set_prop(ctx, key, value);
    }

    pub fn perform_layout_size(
        &mut self,
        constraints: &Constraints,
        children: &[ElementNodeId],
        child_layout: &mut dyn ChildLayout,
    ) -> Size {
        self.inner
            .perform_layout_size(constraints, children, child_layout)
    }

    pub fn perform_layout_position(
        &mut self,
        children: &[ElementNodeId],
        child_layout: &mut dyn ChildLayout,
    ) {
        self.inner.perform_layout_position(children, child_layout);
    }

    pub fn paint(
        &self,
        ctx: &mut dyn PaintContext,
        offset: Offset,
        layout: &ComputedLayout,
        children: &[ElementNodeId],
        child_paint: &mut dyn ChildPaint,
    ) {
        self.inner.paint(ctx, offset, layout, children, child_paint);
    }

    pub fn hit_test(&self, position: Offset, layout: &ComputedLayout) -> bool {
        self.inner.hit_test(position, layout)
    }
}
