use std::cell::Ref;
use std::path::Path;
use std::time::Duration;

use tur_engine::core::element::ElementNodeId;
use tur_engine::core::elements::AnyElement;
use tur_engine::core::elements::ElementTree;
use tur_engine::core::event::{AppEvent, AppGestureEvent, AppImeEvent};
use tur_engine::core::fonts::PresetFontLoader;
use tur_engine::core::keyboard::{AppKeyEvent, KeyEventType, Modifiers};
use tur_engine::elements::PointerInteractElement;
use tur_engine::error::TurError;
use tur_engine::renderer::noop::NoopRenderer;
use tur_engine::TurApp;
use tur_shared::Offset;

pub struct Rect {
    pub left: f64,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
}

impl Rect {
    pub fn center(&self) -> (f64, f64) {
        ((self.left + self.right) / 2.0, (self.top + self.bottom) / 2.0)
    }
}

pub struct TurTestApp {
    inner: TurApp,
}

impl TurTestApp {
    pub fn new(width: f64, height: f64) -> Result<Self, TurError> {
        let mut inner = TurApp::new(
            Box::new(NoopRenderer::new()),
            Box::new(PresetFontLoader::new()),
        )?;
        inner.push_event(AppEvent::Resize {
            logical_width: width as u32,
            logical_height: height as u32,
            dpr: 1.0,
        });
        let _ = inner.spawn_loop_once(Duration::ZERO);
        Ok(Self { inner })
    }

    pub fn load_bundle(&mut self, name: &str) -> Result<(), TurError> {
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        let workspace_root = Path::new(&manifest_dir)
            .parent()
            .and_then(|p| p.parent())
            .expect("failed to resolve workspace root");
        let path = workspace_root
            .join("js/packages/tur-test-cases/dist")
            .join(format!("{name}.js"));
        let source = std::fs::read_to_string(&path).map_err(TurError::Io)?;
        self.inner.load_js(&source)?;
        self.ensure_flushed();
        Ok(())
    }

    pub fn render(&mut self) {
        self.inner.push_event(AppEvent::RequestDraw);
        let _ = self.inner.spawn_loop_once(Duration::ZERO);
    }

    pub fn tick(&mut self) -> Result<(), TurError> {
        self.inner.spawn_loop_once(Duration::ZERO)
    }

    pub fn advance(&mut self, duration: Duration) -> Result<(), TurError> {
        self.inner.spawn_loop_once(duration)
    }

    pub fn element_tree(&self) -> Ref<'_, ElementTree> {
        self.inner.element_tree()
    }

    pub fn click(&mut self, x: f64, y: f64) {
        self.inner
            .push_event(AppEvent::Gesture(AppGestureEvent::PointerDown {
                position: Offset::new(x, y),
            }));
        self.ensure_flushed();
        self.inner
            .push_event(AppEvent::Gesture(AppGestureEvent::PointerUp {
                position: Offset::new(x, y),
            }));
        self.ensure_flushed();
    }

    pub fn send_key(&mut self, key: &str) {
        self.inner.push_event(AppEvent::Key(AppKeyEvent {
            key: key.to_string(),
            code: key.to_string(),
            modifiers: Modifiers::default(),
            event_type: KeyEventType::Down,
        }));
        self.ensure_flushed();
    }

    pub fn send_ime(&mut self, event: AppImeEvent) {
        self.inner.push_event(AppEvent::Ime(event));
        self.ensure_flushed();
    }

    pub fn send_key_with_modifiers(&mut self, key: &str, shift: bool, ctrl: bool) {
        self.inner.push_event(AppEvent::Key(AppKeyEvent {
            key: key.to_string(),
            code: key.to_string(),
            modifiers: Modifiers {
                shift,
                ctrl,
                ..Default::default()
            },
            event_type: KeyEventType::Down,
        }));
        self.ensure_flushed();
    }

    pub fn pointer_down(&mut self, x: f64, y: f64) {
        self.inner
            .push_event(AppEvent::Gesture(AppGestureEvent::PointerDown {
                position: Offset::new(x, y),
            }));
        let _ = self.inner.spawn_loop_once(Duration::ZERO);
    }

    pub fn pointer_move(&mut self, x: f64, y: f64) {
        self.inner
            .push_event(AppEvent::Gesture(AppGestureEvent::PointerMove {
                position: Offset::new(x, y),
            }));
        let _ = self.inner.spawn_loop_once(Duration::ZERO);
    }

    pub fn pointer_up(&mut self, x: f64, y: f64) {
        self.inner
            .push_event(AppEvent::Gesture(AppGestureEvent::PointerUp {
                position: Offset::new(x, y),
            }));
        let _ = self.inner.spawn_loop_once(Duration::ZERO);
    }

    fn ensure_flushed(&mut self) {
        for _ in 0..6 {
            let _ = self.inner.spawn_loop_once(Duration::from_millis(3));
        }
    }

    pub fn has_click_handler(&self, id: ElementNodeId) -> bool {
        self.inner.with_element(id, |e| {
            e.cast::<PointerInteractElement>()
                .map(|p| p.has_on_click())
                .unwrap_or(false)
        }).unwrap_or(false)
    }

    pub fn has_pointer_region_callbacks(&self, id: ElementNodeId) -> bool {
        self.inner.with_element(id, |e| {
            e.cast::<PointerInteractElement>()
                .map(|p| p.has_pointer_region_callbacks())
                .unwrap_or(false)
        }).unwrap_or(false)
    }

    pub fn query_element(&self, key: &[&str]) -> Option<ElementNodeId> {
        self.inner.query_element(key)
    }

    pub fn get_element_absolute_bounds(&self, id: ElementNodeId) -> Option<Rect> {
        let tree = self.inner.element_tree();
        let node = tree.get(id)?;
        let mut x = node.computed_layout.offset.x;
        let mut y = node.computed_layout.offset.y;
        let mut current = node.parent;
        while let Some(cid) = current {
            if let Some(n) = tree.get(cid) {
                x += n.computed_layout.offset.x;
                y += n.computed_layout.offset.y;
                current = n.parent;
            } else {
                break;
            }
        }
        Some(Rect {
            left: x,
            top: y,
            right: x + node.computed_layout.size.width,
            bottom: y + node.computed_layout.size.height,
        })
    }

    pub fn focused_element(&self) -> Option<ElementNodeId> {
        self.inner.focused_element()
    }

    pub fn focused_cursor_rect(&self) -> Option<(f64, f64, f64, f64)> {
        self.inner.focused_cursor_rect()
    }

    pub fn focused_is_editable(&self) -> bool {
        self.inner.focused_is_editable()
    }

    pub fn with_element<R>(
        &self,
        id: ElementNodeId,
        cb: impl FnOnce(&AnyElement) -> R,
    ) -> Option<R> {
        self.inner.with_element(id, cb)
    }

    pub fn eval_js(&mut self, source: &str) -> String {
        self.inner.eval_js(source).unwrap_or_default()
    }

    pub fn load_bundle_source(&mut self, source: &str) -> Result<(), TurError> {
        self.inner.load_js(source)
    }
}
