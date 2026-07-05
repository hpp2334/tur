#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ScrollPhysics {
    Clamping,
}

#[derive(Clone, Debug)]
pub struct ScrollMetrics {
    pub min_scroll_extent: f64,
    pub max_scroll_extent: f64,
    pub pixels: f64,
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
    viewport_size: tur_shared::Size,
    content_size: tur_shared::Size,
    physics: ScrollPhysics,
}

impl Default for ScrollPosition {
    fn default() -> Self {
        Self {
            metrics: ScrollMetrics::default(),
            viewport_size: tur_shared::Size::ZERO,
            content_size: tur_shared::Size::ZERO,
            physics: ScrollPhysics::Clamping,
        }
    }
}

impl ScrollPosition {
    pub fn new() -> Self {
        Self {
            metrics: ScrollMetrics::default(),
            viewport_size: tur_shared::Size::ZERO,
            content_size: tur_shared::Size::ZERO,
            physics: ScrollPhysics::Clamping,
        }
    }

    pub fn apply_dimensions(&mut self, viewport: tur_shared::Size, content: tur_shared::Size) {
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

    pub fn viewport_size(&self) -> tur_shared::Size {
        self.viewport_size
    }

    pub fn content_size(&self) -> tur_shared::Size {
        self.content_size
    }

    pub fn set_extents(&mut self, min: f64, max: f64) {
        self.metrics.min_scroll_extent = min;
        self.metrics.max_scroll_extent = max;
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
