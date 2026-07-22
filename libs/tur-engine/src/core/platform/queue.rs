//! FIFO queue for platform-originated (input) events awaiting dispatch.
//!
//! Drained once per flush iteration by `flush_app_events` and handed to each
//! subsystem via `Subsystem::handle_platform_event`.

use super::event::PlatformEvent;

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
