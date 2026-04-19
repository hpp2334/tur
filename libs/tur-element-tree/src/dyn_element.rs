use std::any::Any;
use std::fmt;

use tur_render_tree::RenderObject;
use tur_trait::ElementKind;

pub trait DynElement: Send + Sync + 'static {
    fn to_render_object_boxed(&self) -> Box<dyn RenderObject>;
    fn kind(&self) -> ElementKind;
    fn name(&self) -> &'static str;
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

pub trait DynElementExt {
    fn cast<T: 'static>(&self) -> Option<&T>;
    fn cast_mut<T: 'static>(&mut self) -> Option<&mut T>;
}

impl DynElementExt for Box<dyn DynElement> {
    fn cast<T: 'static>(&self) -> Option<&T> {
        self.as_any().downcast_ref()
    }

    fn cast_mut<T: 'static>(&mut self) -> Option<&mut T> {
        self.as_any_mut().downcast_mut()
    }
}

impl fmt::Debug for Box<dyn DynElement> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DynElement")
            .field("name", &self.name())
            .field("kind", &self.kind())
            .finish()
    }
}
