pub mod bridge;
pub mod element_bridge;
pub mod elements;

pub use bridge::init_bridge;
pub use element_bridge::{TurAppContext, WeakAppContext};

#[macro_export]
macro_rules! impl_dyn_element {
    ($t:ty) => {
        impl tur_element_tree::DynElement for $t {
            fn to_render_object_boxed(&self) -> Box<dyn tur_render_tree::RenderObject> {
                Box::new(tur_element_tree::Element::to_render_object(self))
            }

            fn kind(&self) -> tur_element_tree::ElementKind {
                tur_element_tree::Element::kind(self)
            }

            fn as_any(&self) -> &dyn std::any::Any {
                self
            }

            fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
                self
            }
        }
    };
}

use boa_engine::object::NativeObject;
use boa_engine::JsObject;
use std::marker::PhantomData;

pub type BoaTurAppContext = BoaOpaque<WeakAppContext>;

#[derive(Debug)]
pub struct BoaOpaque<T> {
    object: JsObject,
    _marker: PhantomData<T>,
}

impl<T> Clone for BoaOpaque<T> {
    fn clone(&self) -> Self {
        Self {
            object: self.object.clone(),
            _marker: PhantomData,
        }
    }
}

impl<T: NativeObject> BoaOpaque<T> {
    pub fn new(data: T, context: &mut boa_engine::Context) -> Self {
        let proto = context.intrinsics().constructors().object().prototype();
        let object = JsObject::from_proto_and_data(proto, data);
        Self {
            object,
            _marker: PhantomData,
        }
    }

    pub fn get(&self) -> boa_engine::JsResult<boa_engine::object::Ref<'_, T>> {
        self.object.downcast_ref::<T>().ok_or_else(|| {
            boa_engine::JsNativeError::typ()
                .with_message("BoaOpaque type mismatch")
                .into()
        })
    }

    pub fn wrap(object: &JsObject) -> Option<boa_engine::object::Ref<'_, T>> {
        object.downcast_ref::<T>()
    }

    pub fn object(&self) -> &JsObject {
        &self.object
    }
}
