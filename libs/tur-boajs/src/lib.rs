pub mod bridge;
pub mod widget_bridge;

pub use bridge::init_bridge;
pub use widget_bridge::TurAppContext;

use boa_engine::object::NativeObject;
use boa_engine::JsObject;
use std::marker::PhantomData;

pub struct BoaOpaque<T> {
    object: JsObject,
    _marker: PhantomData<T>,
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
