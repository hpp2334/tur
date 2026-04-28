use std::cell::Ref;
use std::path::Path;

use tur_engine::core::element::ElementNodeId;
use tur_engine::core::elements::AnyElement;
use tur_engine::core::elements::ElementTree;
use tur_engine::core::event::{AppEvent, AppGestureEvent};
use tur_engine::core::fonts::PresetFontLoader;
use tur_engine::core::gesture::ComposedGestureEventKind;
use tur_engine::core::keyboard::{AppKeyEvent, KeyEventType, Modifiers};
use tur_engine::error::TurError;
use tur_engine::renderer::noop::NoopRenderer;
use tur_engine::TurApp;
use tur_shared::Offset;

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
        let _ = inner.tick();
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
        self.inner.load_js(&source)
    }

    pub fn render(&mut self) {
        self.inner.push_event(AppEvent::RequestDraw);
        let _ = self.inner.tick();
    }

    pub fn tick(&mut self) -> Result<(), TurError> {
        self.inner.tick()
    }

    pub fn element_tree(&self) -> Ref<'_, ElementTree> {
        self.inner.element_tree()
    }

    pub fn click(&mut self, x: f64, y: f64) {
        self.inner
            .push_event(AppEvent::Gesture(AppGestureEvent::PointerDown {
                position: Offset::new(x, y),
            }));
        self.inner
            .push_event(AppEvent::Gesture(AppGestureEvent::PointerUp {
                position: Offset::new(x, y),
            }));
        let _ = self.inner.tick();
    }

    pub fn send_key(&mut self, key: &str) {
        self.inner.push_event(AppEvent::Key(AppKeyEvent {
            key: key.to_string(),
            code: key.to_string(),
            modifiers: Modifiers::default(),
            event_type: KeyEventType::Down,
        }));
        let _ = self.inner.tick();
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
        let _ = self.inner.tick();
    }

    pub fn pointer_down(&mut self, x: f64, y: f64) {
        self.inner
            .push_event(AppEvent::Gesture(AppGestureEvent::PointerDown {
                position: Offset::new(x, y),
            }));
        let _ = self.inner.tick();
    }

    pub fn pointer_move(&mut self, x: f64, y: f64) {
        self.inner
            .push_event(AppEvent::Gesture(AppGestureEvent::PointerMove {
                position: Offset::new(x, y),
            }));
        let _ = self.inner.tick();
    }

    pub fn pointer_up(&mut self, x: f64, y: f64) {
        self.inner
            .push_event(AppEvent::Gesture(AppGestureEvent::PointerUp {
                position: Offset::new(x, y),
            }));
        let _ = self.inner.tick();
    }

    pub fn has_event_handler(&self, id: ElementNodeId, kind: ComposedGestureEventKind) -> bool {
        self.inner.has_event_handler(id, kind)
    }

    pub fn query_element(&self, key: &[&str]) -> Option<ElementNodeId> {
        self.inner.query_element(key)
    }

    pub fn with_element<R>(
        &self,
        id: ElementNodeId,
        cb: impl FnOnce(&AnyElement) -> R,
    ) -> Option<R> {
        self.inner.with_element(id, cb)
    }
}
