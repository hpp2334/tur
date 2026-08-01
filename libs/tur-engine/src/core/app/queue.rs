//! FIFO queue for engine-internal events (programmatic scrolls, clipboard
//! writes, paste forwarding) produced by elements/handlers during a flush.
//! Re-drained each iteration of the fixed-point flush loop until quiescence.

use super::event::AppEvent;

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
        Self { events: Vec::new() }
    }

    pub fn push(&mut self, event: AppEvent) {
        self.events.push(event);
    }

    pub fn drain(&mut self) -> Vec<AppEvent> {
        std::mem::take(&mut self.events)
    }
}
