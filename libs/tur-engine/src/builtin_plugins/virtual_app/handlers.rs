//! `VirtualAppSubsystem` — the worker-side half of the seam. Consumes
//! status + frame events arriving host→worker on the `AppEvent::Custom`
//! rail, stores child outputs for the host element's paint, ships
//! layout-driven `Resize` controls from `flush_post_layout` (final
//! geometry — the `CompositedTransformSubsystem` precedent), and forwards
//! pointer/wheel input over a host element into its child.

use std::rc::Rc;

use crate::core::app::AppEvent;
use crate::core::elements::NodeTreeData;
use crate::core::hit_test::HitTest;
use crate::core::js_runtime::TurInstanceContext;
use crate::core::layout::Offset;
use crate::core::platform::PlatformEvent;
use crate::core::shell::{PointerInput, ShellEvent};
use crate::core::subsystem::{Subsystem, SubsystemFlushContext};
use crate::core::virtual_app::{VirtualControl, VirtualFrameEvent, VirtualStatusEvent};

use super::element::VirtualAppElement;
use super::state::VirtualState;

pub(crate) struct VirtualAppSubsystem {
    state: Rc<VirtualState>,
    js_ctx: TurInstanceContext,
}

impl VirtualAppSubsystem {
    pub(crate) fn new(state: Rc<VirtualState>, js_ctx: TurInstanceContext) -> Self {
        Self { state, js_ctx }
    }

    /// Ship the host element's final rect to its child (deduped per
    /// controller record). Only walks while a controller is bound.
    ///
    /// NOTE: the first rect often races the spawn (the `Resize` control is
    /// dropped host-side for a token with no child yet) — `handle_status`
    /// resets `last_rect` when a child reaches `Running`, so the next
    /// flush re-ships the rect against the now-live child.
    fn ship_rects(&self, tree: &NodeTreeData) {
        let Some(root) = tree.root_element_id() else {
            return;
        };
        let mut stack = vec![root];
        while let Some(id) = stack.pop() {
            let Some(node) = tree.get_element(id) else {
                continue;
            };
            if let Some(element) = node.element.as_ref()
                && let Some(el) = element.cast::<VirtualAppElement>()
                && let Some(app) = el.painting.app.as_ref()
                && let Some(record) = self.state.record(app.0)
            {
                let affine = tree.absolute_affine_of(id);
                let t = affine.translation();
                let size = node.computed_layout.size;
                let rect = (t.x, t.y, size.width, size.height);
                if record.last_rect.get() != rect {
                    record.last_rect.set(rect);
                    if let Some(token) = record.current.get() {
                        self.state.send_control(VirtualControl::Resize {
                            token,
                            x: rect.0,
                            y: rect.1,
                            width: rect.2,
                            height: rect.3,
                            dpr: 1.0,
                        });
                    }
                }
            }
            stack.extend(tree.children_of_element(id));
        }
    }

    /// Forward position-carrying input over a host element into its child.
    ///
    /// The hit path is walked front-to-back: the FIRST virtual host in the
    /// path receives the event, translated into child-local coordinates
    /// (`position − host origin` — child-viewport space maps 1:1 onto the
    /// element rect). If an interactive element (mouse region / pointer
    /// interact) sits in front of every host, it consumes the event and the
    /// child sees nothing. The child composes gestures in its own arena;
    /// the parent never dispatches on the child's behalf. Key/IME events do
    /// not participate (child focus is a later milestone).
    fn forward_input(&self, cx: &SubsystemFlushContext<'_>, event: &PlatformEvent) {
        let Some(global) = input_position(event) else {
            return;
        };
        let tree = cx.element_tree.borrow();
        let path = HitTest::new(&tree).path(global);
        for id in path {
            let Some(node) = tree.get_element(id) else {
                continue;
            };
            let Some(element) = node.element.as_ref() else {
                continue;
            };
            if let Some(host) = element.cast::<VirtualAppElement>() {
                let Some(app) = host.painting.app.as_ref() else {
                    continue;
                };
                let Some(record) = self.state.record(app.0) else {
                    continue;
                };
                let Some(token) = record.current.get() else {
                    // Idle / destroyed host — not a consumer; keep walking
                    // in case another host (or interactive element) is
                    // behind it in the path.
                    continue;
                };
                let Some(translated) = translate_input(event, local_position(&tree, id, global))
                else {
                    return;
                };
                self.state.send_control(VirtualControl::PlatformEvent {
                    token,
                    event: PlatformEvent::Shell(translated),
                });
                return;
            }
            if element
                .cast::<crate::builtin_plugins::gesture::MouseRegionElement>()
                .is_some()
                || element
                    .cast::<crate::builtin_plugins::gesture::PointerInteractElement>()
                    .is_some()
            {
                // An interactive element covers every host — gesture wins.
                return;
            }
        }
    }
}

/// The viewport position a position-carrying input event reports (pointer
/// down/up/move + wheel). `None` for everything else (keys, IME, …).
fn input_position(event: &PlatformEvent) -> Option<Offset> {
    match event {
        PlatformEvent::Shell(ShellEvent::Pointer(
            PointerInput::PointerDown { position, .. }
            | PointerInput::PointerUp { position, .. }
            | PointerInput::PointerMove { position, .. },
        )) => Some(*position),
        PlatformEvent::Shell(ShellEvent::Wheel { position, .. }) => Some(*position),
        _ => None,
    }
}

/// Child-local coordinates for a point over `id`'s rect (position − the
/// element's absolute origin — child-viewport space maps 1:1 onto the
/// element rect).
fn local_position(
    tree: &NodeTreeData,
    id: crate::core::element::ElementNodeId,
    global: Offset,
) -> Offset {
    let t = tree.absolute_affine_of(id).translation();
    Offset::new(global.x - t.x, global.y - t.y)
}

/// Translate a position-carrying input event into child-local coordinates
/// (a fresh [`ShellEvent`] — no clone of the borrowed event needed).
/// `None` for anything [`input_position`] rejects; callers pre-filter.
fn translate_input(event: &PlatformEvent, local: Offset) -> Option<ShellEvent> {
    Some(match event {
        PlatformEvent::Shell(ShellEvent::Pointer(PointerInput::PointerDown {
            position: _,
            button,
            time_ms,
            device,
        })) => ShellEvent::Pointer(PointerInput::PointerDown {
            position: local,
            button: *button,
            time_ms: *time_ms,
            device: *device,
        }),
        PlatformEvent::Shell(ShellEvent::Pointer(PointerInput::PointerUp {
            position: _,
            button,
            time_ms,
            device,
        })) => ShellEvent::Pointer(PointerInput::PointerUp {
            position: local,
            button: *button,
            time_ms: *time_ms,
            device: *device,
        }),
        PlatformEvent::Shell(ShellEvent::Pointer(PointerInput::PointerMove {
            position: _,
            time_ms,
            device,
        })) => ShellEvent::Pointer(PointerInput::PointerMove {
            position: local,
            time_ms: *time_ms,
            device: *device,
        }),
        PlatformEvent::Shell(ShellEvent::Wheel {
            delta_x,
            delta_y,
            position: _,
        }) => ShellEvent::Wheel {
            delta_x: *delta_x,
            delta_y: *delta_y,
            position: local,
        },
        _ => return None,
    })
}

impl Subsystem for VirtualAppSubsystem {
    fn flush_post_layout(&mut self, cx: &mut SubsystemFlushContext<'_>) {
        if !self.state.any_bound() {
            return;
        }
        let tree = cx.element_tree.clone();
        let tree = tree.borrow();
        self.ship_rects(&tree);
    }

    fn handle_platform_event(&mut self, cx: &mut SubsystemFlushContext<'_>, event: &PlatformEvent) {
        if self.state.any_bound() {
            self.forward_input(cx, event);
        }
    }

    fn handle_app_event(&mut self, cx: &mut SubsystemFlushContext<'_>, event: &AppEvent) {
        if let Some(status) = event.as_custom::<VirtualStatusEvent>() {
            self.state
                .handle_status(status.token, status.state, status.detail.as_deref());
            // Status flips are reactive (`set_source`) — subscribed elements
            // re-layout through the ordinary invalidation rail.
        }
        if let Some(frame) = event.as_custom::<VirtualFrameEvent>() {
            let js_ctx = self.js_ctx.clone();
            self.state.store_frame(
                frame.token,
                frame.batch.clone(),
                frame.images.clone(),
                |image| js_ctx.register_image(image),
            );
            cx.request_paint();
        }
    }
}
