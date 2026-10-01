use std::cell::RefCell;
use std::rc::Rc;

use crate::builtin_plugins::layout::ContainerView;
use crate::builtin_plugins::text::controller::{TextEditingController, UndoController};
use crate::core::element::NodeId;
use crate::core::render::brush::Color;
use crate::core::view::{Val, View, ViewCx};

use super::element::{ContextMenuEvent, EditableTextView};

// ---------------------------------------------------------------------------
// InputView — composes a ContainerElement (sizing/border wrapper) with a single
// EditableTextElement child. Input is NOT its own element; it's a spec that builds
// a ContainerElement + EditableTextElement subtree.
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct InputView {
    width: Option<Val<f64>>,
    height: Option<Val<f64>>,
    controller: Option<Rc<RefCell<TextEditingController>>>,
    undo_controller: Option<Rc<RefCell<UndoController>>>,
    placeholder: Option<Val<String>>,
    color: Option<Val<Color>>,
    placeholder_color: Option<Val<Color>>,
    cursor_color: Option<Val<Color>>,
    font_size: Option<Val<f64>>,
    font_family: Option<Val<String>>,
    font_weight: Option<Val<f64>>,
    multiline: Option<Val<bool>>,
    obscure_text: Option<Val<bool>>,
    obscuring_character: Option<Val<String>>,
    on_context_menu: Option<crate::core::edgy::mutation::MutationHandle<ContextMenuEvent>>,
    query_key: Option<Vec<String>>,
}

impl View for InputView {
    fn build(&self, cx: &mut dyn ViewCx, parent: NodeId) -> NodeId {
        let editable = Rc::new(EditableTextView {
            controller: self.controller.clone(),
            undo_controller: self.undo_controller.clone(),
            placeholder: self.placeholder.clone(),
            color: self.color.clone(),
            placeholder_color: self.placeholder_color.clone(),
            cursor_color: self.cursor_color.clone(),
            font_size: self.font_size.clone(),
            font_family: self.font_family.clone(),
            font_weight: self.font_weight.clone(),
            multiline: self.multiline.clone(),
            obscure_text: self.obscure_text.clone(),
            obscuring_character: self.obscuring_character.clone(),
            on_context_menu: self.on_context_menu,
            query_key: None,
        });
        let container_spec = ContainerView {
            width: self.width.clone(),
            height: self.height.clone(),
            children: vec![editable],
            query_key: self.query_key.clone(),
            ..Default::default()
        };
        container_spec.build(cx, parent)
    }
}

impl InputView {
    /// An all-defaults builder (`core::rut_runtime`'s `el_input_new` row):
    /// every prop unset; the setter rows mutate it in place and `el_build`
    /// materializes it.
    pub(crate) fn empty_rut() -> Self {
        InputView {
            width: None,
            height: None,
            controller: None,
            undo_controller: None,
            placeholder: None,
            color: None,
            placeholder_color: None,
            cursor_color: None,
            font_size: None,
            font_family: None,
            font_weight: None,
            multiline: None,
            obscure_text: None,
            obscuring_character: None,
            on_context_menu: None,
            query_key: None,
        }
    }

    // -- rut builder setters (`core::rut_runtime`'s input_* rows) ---------

    pub(crate) fn set_width(&mut self, v: f64) {
        self.width = Some(Val::Static(v));
    }
    pub(crate) fn set_height(&mut self, v: f64) {
        self.height = Some(Val::Static(v));
    }
    pub(crate) fn set_placeholder_str(&mut self, v: String) {
        self.placeholder = Some(Val::Static(v));
    }
    pub(crate) fn set_color(&mut self, v: crate::core::render::brush::Color) {
        self.color = Some(Val::Static(v));
    }
    pub(crate) fn set_placeholder_color(&mut self, v: crate::core::render::brush::Color) {
        self.placeholder_color = Some(Val::Static(v));
    }
    pub(crate) fn set_font_size(&mut self, v: f64) {
        self.font_size = Some(Val::Static(v));
    }
    pub(crate) fn set_controller(&mut self, c: Rc<RefCell<TextEditingController>>) {
        self.controller = Some(c);
    }
    pub(crate) fn set_query_key(&mut self, key: Vec<String>) {
        self.query_key = Some(key);
    }

    /// Rut-rail constructor (`core::rut_runtime`): shared controllers, a
    /// static placeholder, and an explicit size — the rows the rut C1 gate
    /// drives. Everything else defaults (matching an un-styled `Input`).
    pub(crate) fn new_rut(
        controller: Rc<RefCell<TextEditingController>>,
        undo_controller: Option<Rc<RefCell<UndoController>>>,
        placeholder: Option<String>,
        width: Option<f64>,
        height: Option<f64>,
    ) -> Self {
        InputView {
            width: width.map(Val::Static),
            height: height.map(Val::Static),
            controller: Some(controller),
            undo_controller,
            placeholder: placeholder.map(Val::Static),
            color: None,
            placeholder_color: None,
            cursor_color: None,
            font_size: None,
            font_family: None,
            font_weight: None,
            multiline: None,
            obscure_text: None,
            obscuring_character: None,
            on_context_menu: None,
            query_key: Some(vec!["rut".to_string(), "input".to_string()]),
        }
    }

    /// Rut-rail constructor with option flags (bit 0 = multiline,
    /// bit 1 = obscure) — the `el_input_opts` row's crossing.
    pub(crate) fn new_rut_opts(
        controller: Rc<RefCell<TextEditingController>>,
        undo_controller: Option<Rc<RefCell<UndoController>>>,
        placeholder: Option<String>,
        width: Option<f64>,
        height: Option<f64>,
        multiline: bool,
        obscure: bool,
    ) -> Self {
        let mut view = Self::new_rut(controller, undo_controller, placeholder, width, height);
        if multiline {
            view.multiline = Some(Val::Static(true));
        }
        if obscure {
            view.obscure_text = Some(Val::Static(true));
        }
        view
    }
}
