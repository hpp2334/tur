#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ScrollPhysics {
    Clamping,
}

#[derive(Clone, Debug)]
struct ScrollMetrics {
    min_scroll_extent: f64,
    max_scroll_extent: f64,
    pixels: f64,
}

impl Default for ScrollMetrics {
    fn default() -> Self {
        Self {
            min_scroll_extent: 0.0,
            max_scroll_extent: 0.0,
            pixels: 0.0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ScrollPosition {
    metrics: ScrollMetrics,
    viewport_size: crate::core::layout::Size,
    content_size: crate::core::layout::Size,
    physics: ScrollPhysics,
}

impl Default for ScrollPosition {
    fn default() -> Self {
        Self {
            metrics: ScrollMetrics::default(),
            viewport_size: crate::core::layout::Size::ZERO,
            content_size: crate::core::layout::Size::ZERO,
            physics: ScrollPhysics::Clamping,
        }
    }
}

impl ScrollPosition {
    pub fn new() -> Self {
        Self {
            metrics: ScrollMetrics::default(),
            viewport_size: crate::core::layout::Size::ZERO,
            content_size: crate::core::layout::Size::ZERO,
            physics: ScrollPhysics::Clamping,
        }
    }

    pub fn apply_dimensions(
        &mut self,
        viewport: crate::core::layout::Size,
        content: crate::core::layout::Size,
    ) {
        self.viewport_size = viewport;
        self.content_size = content;
    }

    pub fn apply_scroll_delta(&mut self, delta: f64) -> f64 {
        let new_pixels = self.metrics.pixels + delta;
        let overscroll = self.apply_boundary_conditions(new_pixels);
        let clamped = new_pixels.clamp(
            self.metrics.min_scroll_extent,
            self.metrics.max_scroll_extent,
        );
        self.metrics.pixels = clamped;
        overscroll
    }

    pub fn correct_pixels(&mut self, value: f64) {
        self.metrics.pixels = value;
    }

    pub fn pixels(&self) -> f64 {
        self.metrics.pixels
    }

    pub fn max_scroll_extent(&self) -> f64 {
        self.metrics.max_scroll_extent
    }

    pub fn viewport_size(&self) -> crate::core::layout::Size {
        self.viewport_size
    }

    pub fn content_size(&self) -> crate::core::layout::Size {
        self.content_size
    }

    /// Write the new scroll extents AND clamp `pixels` into them — Flutter
    /// `applyContentDimensions` parity. Called from layout each pass with
    /// the freshly-measured content; without the clamp, content that shrinks
    /// below the current offset leaves the viewport scrolled past the
    /// content end (blank viewport) until the next wheel/drag delta.
    ///
    /// Returns `true` when the clamp moved `pixels` (callers sync controller
    /// metrics and fire `onScroll` for the correction in that case).
    pub fn set_extents(&mut self, min: f64, max: f64) -> bool {
        debug_assert!(min <= max, "scroll extents must be ordered");
        self.metrics.min_scroll_extent = min;
        self.metrics.max_scroll_extent = max;
        let clamped = self.metrics.pixels.clamp(min, max);
        if clamped != self.metrics.pixels {
            self.metrics.pixels = clamped;
            true
        } else {
            false
        }
    }

    fn apply_boundary_conditions(&self, value: f64) -> f64 {
        match self.physics {
            ScrollPhysics::Clamping => {
                if value < self.metrics.min_scroll_extent {
                    value - self.metrics.min_scroll_extent
                } else if value > self.metrics.max_scroll_extent {
                    value - self.metrics.max_scroll_extent
                } else {
                    0.0
                }
            }
        }
    }
}
