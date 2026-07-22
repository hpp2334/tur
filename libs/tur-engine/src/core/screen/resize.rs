use crate::core::platform::PlatformEvent;
use crate::core::subsystem::{Subsystem, SubsystemFlushContext};

pub struct ResizeSubsystem;

impl Subsystem for ResizeSubsystem {
    fn handle_platform_event(
        &mut self,
        cx: &mut SubsystemFlushContext<'_>,
        event: &PlatformEvent,
    ) {
        let PlatformEvent::Resize {
            logical_width,
            logical_height,
            dpr,
        } = event
        else {
            return;
        };

        cx.renderer
            .resize(*logical_width, *logical_height, *dpr);
        *cx.screen_logical_size = (*logical_width as f64, *logical_height as f64);
        cx.element_tree.borrow_mut().mark_root_dirty();
        cx.request_paint();
    }
}
