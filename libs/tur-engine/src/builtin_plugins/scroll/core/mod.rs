pub mod controller;

pub use controller::ScrollController;

// ---------------------------------------------------------------------------
// Scroll event payload — callback arguments for onScroll.
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct ScrollEvent {
    pub(crate) offset: f64,
    pub(crate) max_extent: f64,
    pub(crate) viewport_dimension: f64,
}

impl ScrollEvent {
    /// Construct a scroll event payload from the controller's live metrics.
    pub fn new(offset: f64, max_extent: f64, viewport_dimension: f64) -> Self {
        Self {
            offset,
            max_extent,
            viewport_dimension,
        }
    }
}

impl crate::core::edgy::mutation::MutationPayload for ScrollEvent {
    /// The scalar metrics — `[offset, maxExtent, viewportDimension]`.
    fn to_value_args(&self) -> Vec<crate::core::edgy::Value> {
        vec![
            crate::core::edgy::Value::Num(self.offset),
            crate::core::edgy::Value::Num(self.max_extent),
            crate::core::edgy::Value::Num(self.viewport_dimension),
        ]
    }
}
