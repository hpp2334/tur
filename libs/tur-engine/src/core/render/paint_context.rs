use std::time::Duration;

use tur_shared::{Cursor, Offset, Size};

use crate::core::element::ElementNodeId;
use crate::core::elements::ElementTree;
use crate::core::render::Canvas;
use crate::core::resource::{ImageResource, ResourceId, ResourceMap};
use crate::core::shell::PaintShell;

pub struct PaintContext<'a> {
    tree: &'a ElementTree,
    resource_map: &'a ResourceMap,
    focused_node_id: Option<ElementNodeId>,
    current_node_id: Option<ElementNodeId>,
    /// Shell face for this paint pass: cursor claims, time, pointer position.
    /// See [`PaintShell`] for the (deliberately limited) surface.
    shell: PaintShell<'a>,
}

impl<'a> PaintContext<'a> {
    pub(crate) fn new(
        tree: &'a ElementTree,
        focused_node_id: Option<ElementNodeId>,
        current_node_id: ElementNodeId,
        resource_map: &'a ResourceMap,
        shell: PaintShell<'a>,
    ) -> Self {
        PaintContext {
            tree,
            resource_map,
            focused_node_id,
            current_node_id: Some(current_node_id),
            shell,
        }
    }

    pub fn paint_child(
        &self,
        child_id: ElementNodeId,
        canvas: &mut dyn Canvas,
        parent_offset: Offset,
    ) {
        self.tree.paint_node(
            child_id,
            canvas,
            parent_offset,
            self.focused_node_id,
            self.resource_map,
            self.shell,
        );
    }

    pub fn is_focused(&self) -> bool {
        self.focused_node_id == self.current_node_id
    }

    /// Current frame time as a `Duration` since the epoch. Used by time-based
    /// paint effects (e.g. caret blink phase).
    pub fn now(&self) -> Duration {
        self.shell.now()
    }

    pub fn get_image_resource(&self, id: ResourceId) -> Option<&ImageResource> {
        self.resource_map.get_image(id)
    }

    /// True if the last known pointer position lies within the rectangle at
    /// `offset` with the given `size` (both in canvas-local logical pixels).
    /// Returns `false` when no pointer position is known.
    pub fn pointer_inside(&self, offset: Offset, size: &Size) -> bool {
        let Some(p) = self.shell.pointer_position() else {
            return false;
        };
        p.x >= offset.x
            && p.x < offset.x + size.width
            && p.y >= offset.y
            && p.y < offset.y + size.height
    }

    /// Claim the host cursor for this frame. See [`PaintShell::set_cursor`].
    pub fn set_cursor(&self, cursor: Cursor) {
        self.shell.set_cursor(cursor);
    }
}
