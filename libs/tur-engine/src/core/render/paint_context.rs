use std::time::Duration;

use vello_common::kurbo::{Affine, Point};

use crate::core::layout::Size;
use crate::core::platform::Cursor;

use crate::core::element::ElementNodeId;
use crate::core::elements::NodeTreeData;
use crate::core::image_resource::{ImageMetadataMap, ImageResourceId};
use crate::core::render::Canvas;
use crate::core::shell::PaintShell;

pub struct PaintContext<'a> {
    tree: &'a NodeTreeData,
    image_metadata_map: &'a ImageMetadataMap,
    focused_node_id: Option<ElementNodeId>,
    current_node_id: Option<ElementNodeId>,
    /// Shell face for this paint pass: cursor claims, time, pointer position.
    /// See [`PaintShell`] for the (deliberately limited) surface.
    shell: PaintShell<'a>,
    /// This node's absolute (world) affine — the product of its ancestors'
    /// `relative_transform` and its own. The paint walk pushes each node's
    /// `relative_transform` onto the canvas transform stack, so element `paint`
    /// bodies draw in their own local space; this field lets them map
    /// canvas-space quantities (e.g. the pointer position) back into local
    /// space (see [`Self::pointer_inside`]).
    current_transform: Affine,
}

impl<'a> PaintContext<'a> {
    pub fn new(
        tree: &'a NodeTreeData,
        focused_node_id: Option<ElementNodeId>,
        current_node_id: ElementNodeId,
        image_metadata_map: &'a ImageMetadataMap,
        shell: PaintShell<'a>,
        current_transform: Affine,
    ) -> Self {
        PaintContext {
            tree,
            image_metadata_map,
            focused_node_id,
            current_node_id: Some(current_node_id),
            shell,
            current_transform,
        }
    }

    /// Paint a child. The child's `relative_transform` is pushed onto the
    /// canvas transform stack inside [`NodeTreeData::paint_element`], so no
    /// offset is passed — the child paints in its own local space.
    pub fn paint_child(&self, child_id: ElementNodeId, canvas: &mut dyn Canvas) {
        self.tree.paint_element(
            child_id,
            canvas,
            self.current_transform,
            self.focused_node_id,
            self.image_metadata_map,
            self.shell,
        );
    }

    pub fn is_focused(&self) -> bool {
        self.focused_node_id == self.current_node_id
    }

    /// Current frame time as a `Duration` since the epoch. Used by
    /// time-based paint effects.
    pub fn now(&self) -> Duration {
        self.shell.now()
    }

    /// Natural size of a registered image resource. Paint reads the size
    /// (BoxFit math + `Canvas::draw_image(rid, size, …)`); the pixel data
    /// lives on main, not in this map.
    pub fn get_image_size(&self, id: ImageResourceId) -> Option<Size> {
        self.image_metadata_map.get(&id).map(|m| m.size)
    }

    /// True if the last known pointer position (canvas-space) lies within this
    /// node's local `[0, size]` box. The pointer is mapped into local space
    /// through the inverse of [`Self::current_transform`], so this is correct
    /// for translated / rotated / scaled subtrees (used for cursor claims).
    /// Returns `false` when no pointer position is known.
    pub fn pointer_inside(&self, size: &Size) -> bool {
        let Some(p) = self.shell.pointer_position() else {
            return false;
        };
        let local = self.current_transform.inverse() * Point::new(p.x, p.y);
        local.x >= 0.0 && local.x < size.width && local.y >= 0.0 && local.y < size.height
    }

    /// Claim the host cursor for this frame. See [`PaintShell::set_cursor`].
    pub fn set_cursor(&self, cursor: Cursor) {
        self.shell.set_cursor(cursor);
    }
}
