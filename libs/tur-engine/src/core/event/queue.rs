use super::{AppEvent, PlatformEvent};

/// FIFO queue for platform-originated (input) events awaiting dispatch.
/// Drained once per flush iteration by `flush_app_events` and handed to each
/// handler via [`AppHandler::handle_platform_event`](crate::core::handler::AppHandler::handle_platform_event).
pub struct PlatformEventQueue {
    events: Vec<PlatformEvent>,
}

impl Default for PlatformEventQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl PlatformEventQueue {
    pub fn new() -> Self {
        Self {
            events: Vec::new(),
        }
    }

    pub fn push(&mut self, event: PlatformEvent) {
        self.events.push(event);
    }

    pub fn drain(&mut self) -> Vec<PlatformEvent> {
        std::mem::take(&mut self.events)
    }
}

/// FIFO queue for engine-internal events (draw requests, programmatic
/// scrolls, clipboard writes) produced by elements/handlers during a flush.
/// Re-drained each iteration of the fixed-point flush loop until quiescence.
pub struct AppEventQueue {
    events: Vec<AppEvent>,
}

impl Default for AppEventQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl AppEventQueue {
    pub fn new() -> Self {
        Self {
            events: Vec::new(),
        }
    }

    pub fn push(&mut self, event: AppEvent) {
        self.events.push(event);
    }

    pub fn drain(&mut self) -> Vec<AppEvent> {
        std::mem::take(&mut self.events)
    }
}
