pub mod bridge;
pub mod widget_bridge;

pub use bridge::init_bridge;
pub use widget_bridge::TurAppContext;

use boa_gc::{empty_trace, Finalize, Trace};
use std::ops::{Deref, DerefMut};

#[derive(Debug, Clone, Default, Finalize)]
pub struct BoaOpaque<T>(pub T);

unsafe impl<T> Trace for BoaOpaque<T> {
    empty_trace!();
}

impl<T> Deref for BoaOpaque<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for BoaOpaque<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
