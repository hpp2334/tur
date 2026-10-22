//! `VirtualAppSubsystem` — the worker-side half of the seam. Consumes
//! status + frame events arriving host→worker on the `AppEvent::Custom`
//! rail, stores child outputs for the host element's paint, ships
//! layout-driven `Resize` controls from `flush_post_layout` (final
//! geometry — the `CompositedTransformSubsystem` precedent), and forwards
//! pointer/wheel input over a host element into its child.

use std::rc::Rc;

use crate::core::app::AppEvent;
use crate::core::app::comm::ShellCommand;
use crate::core::element::ElementNodeId;
use crate::core::elements::NodeTreeData;
use crate::core::hit_test::HitTest;
use crate::core::instance::InstanceContext;
use crate::core::layout::Offset;
use crate::core::platform::PlatformEvent;
use crate::core::shell::{PointerInput, ShellEvent, TextInputState};
use crate::core::subsystem::{Subsystem, SubsystemFlushContext};
use crate::core::virtual_app::{
    VirtualControl, VirtualErrorEvent, VirtualFocusEvent, VirtualFrameEvent, VirtualStatusEvent,
    VirtualTextInputEvent,
};

use super::element::VirtualAppElement;
use super::state::VirtualState;

pub(crate) struct VirtualAppSubsystem {
    state: Rc<VirtualState>,
    instance: InstanceContext,
}

impl VirtualAppSubsystem {
    pub(crate) fn new(state: Rc<VirtualState>, instance: InstanceContext) -> Self {
        Self { state, instance }
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
    /// not ride this walk — they are focus-routed, not position-routed (see
    /// `forward_key_event`).
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

    /// Forward key/IME input into the child that holds focus.
    ///
    /// Keys are FOCUS-routed, not position-routed: the parent's focus
    /// manager holds the host element's id while the child is focused (the
    /// child reports focus in/out via [`VirtualFocusEvent`]), so a focused
    /// host resolves to the child's live token and the event crosses
    /// UNTRANSLATED (key/IME events carry no position). When a parent
    /// element holds focus instead, nothing resolves here and the ordinary
    /// key subsystems serve it — the two routes are exclusive.
    fn forward_key_event(&self, cx: &SubsystemFlushContext<'_>, event: &PlatformEvent) {
        let Some(translated) = key_event_payload(event) else {
            return;
        };
        let focused = cx.focus_manager.borrow().focused();
        let Some(token) = focused.and_then(|host| self.state.focused_child_token(host)) else {
            return;
        };
        self.state.send_control(VirtualControl::PlatformEvent {
            token,
            event: PlatformEvent::Shell(translated),
        });
    }

    /// The child reported a focus change (`inside` = its focus manager now
    /// holds / no longer holds an element). The parent's focus manager
    /// LEARNS this: the host element id — the record's binder — is set as
    /// the focused node, so
    ///
    /// - a parent element holding focus is blurred first (`set_focus`
    ///   blur-notification) — child and parent focus are exclusive, and
    /// - a later pointer click elsewhere clears the host via the ordinary
    ///   gesture sweep (the host is has_focus() + Focusable), whose blur
    ///   notification forwards `ClearFocus` into the child.
    fn handle_child_focus(
        &self,
        cx: &SubsystemFlushContext<'_>,
        token: crate::core::virtual_app::VirtualAppId,
        inside: bool,
    ) {
        let Some(record) = self.state.record_by_token(token) else {
            return; // retired incarnation — nothing to inform
        };
        if record.current.get() != Some(token) {
            return;
        }
        let Some(binder) = record.binder.get() else {
            return;
        };
        let host = ElementNodeId::new(binder);
        let mut focus = cx.focus_manager.borrow_mut();
        if inside {
            if focus.focused() != Some(host) {
                focus.set_focus(host);
            }
        } else if focus.focused() == Some(host) {
            focus.clear_focus();
        }
    }

    /// The child's deduped text-input egress: re-ship it against THIS
    /// instance's shell with the caret rect translated out of
    /// child-viewport space (the host element's absolute rect is the
    /// mapping — child-viewport coordinates map 1:1 onto it). This is what
    /// raises/positions the embedder's IME surface (the browser's hidden
    /// textarea) while a child editable is focused.
    fn forward_text_input(
        &self,
        token: crate::core::virtual_app::VirtualAppId,
        state: &TextInputState,
    ) {
        let Some(record) = self.state.record_by_token(token) else {
            return;
        };
        if record.current.get() != Some(token) {
            return;
        }
        let (hx, hy, _, _) = record.last_rect.get();
        let translated = TextInputState {
            is_editable: state.is_editable,
            cursor_rect: state
                .cursor_rect
                .map(|(x, y, w, h)| (hx + x, hy + y, w, h)),
        };
        self.state.ship_shell(ShellCommand::RequestTextInput(translated));
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

/// A fresh Key/IME [`ShellEvent`] cloned out of a platform event — the
/// focus-routed forwarder's payload. `None` for everything else.
fn key_event_payload(event: &PlatformEvent) -> Option<ShellEvent> {
    match event {
        PlatformEvent::Shell(ShellEvent::Key(key)) => Some(ShellEvent::Key(key.clone())),
        PlatformEvent::Shell(ShellEvent::Ime(ime)) => Some(ShellEvent::Ime(ime.clone())),
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
            self.forward_key_event(cx, event);
        }
    }

    fn handle_app_event(&mut self, cx: &mut SubsystemFlushContext<'_>, event: &AppEvent) {
        if let Some(focus) = event.as_custom::<VirtualFocusEvent>() {
            self.handle_child_focus(cx, focus.token, focus.inside);
        }
        if let Some(text_input) = event.as_custom::<VirtualTextInputEvent>() {
            self.forward_text_input(text_input.token, &text_input.state);
        }
        if let Some(status) = event.as_custom::<VirtualStatusEvent>() {
            self.state
                .handle_status(status.token, status.state, status.detail.as_deref());
            // Status flips are reactive (`set_source`) — subscribed elements
            // re-layout through the ordinary invalidation rail.
        }
        if let Some(frame) = event.as_custom::<VirtualFrameEvent>() {
            let instance = self.instance.clone();
            self.state.store_frame(
                frame.token,
                frame.batch.clone(),
                frame.images.clone(),
                |image| instance.register_image(image),
            );
            cx.request_paint();
        }
        if let Some(error) = event.as_custom::<VirtualErrorEvent>() {
            // Same-frame dispatch: the mutation queue drains later in this
            // fixed-point iteration, and `handled_events` keeps the loop
            // alive. A throwing parent callback is itself logged (and
            // forwarded up the chain if this parent is itself a child).
            self.state.handle_runtime_error(
                error.token,
                error.report.clone(),
                cx.frame_id(),
                &cx.mutation_queue,
            );
        }
    }
}
