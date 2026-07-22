use std::rc::Rc;

use boa_engine::object::JsObject;
use boa_engine::Context;
use crate::core::render::brush::Color;

use crate::core::js_runtime::JsProps;
use crate::core::element::NodeId;
use crate::core::view::{ViewCx, View, Val};
use crate::core::edgy::reactive::AnyReadable;
use crate::builtin_plugins::layout::ContainerView;

use super::element::{ContextMenuEvent, EditableTextView};
use crate::builtin_plugins::text::controller::{TextEditingController, UndoController};

// ---------------------------------------------------------------------------
// InputView — composes a ContainerElement (sizing/border wrapper) with a single
// EditableTextElement child. Input is NOT its own element; it's a spec that builds
// a ContainerElement + EditableTextElement subtree.
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct InputView {
    width: Option<Val<f64>>,
    height: Option<Val<f64>>,
    controller: Option<JsObject>,
    controller_atom: Option<AnyReadable>,
    undo_controller: Option<JsObject>,
    placeholder: Option<Val<String>>,
    color: Option<Val<Color>>,
    placeholder_color: Option<Val<Color>>,
    cursor_color: Option<Val<Color>>,
    font_size: Option<Val<f64>>,
    font_family: Option<Val<String>>,
    multiline: Option<Val<bool>>,
    on_context_menu: Option<crate::core::edgy::mutation::MutationHandle<ContextMenuEvent>>,
    query_key: Option<Vec<String>>,
}

impl View for InputView {
    fn build(&self, cx: &mut dyn ViewCx, boa: &mut Context, parent: NodeId) -> NodeId {
        let editable = Rc::new(EditableTextView {
            controller: self.controller.clone(),
            controller_atom: self.controller_atom,
            undo_controller: self.undo_controller.clone(),
            placeholder: self.placeholder.clone(),
            color: self.color.clone(),
            placeholder_color: self.placeholder_color.clone(),
            cursor_color: self.cursor_color.clone(),
            font_size: self.font_size.clone(),
            font_family: self.font_family.clone(),
            multiline: self.multiline.clone(),
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
        container_spec.build(cx, boa, parent)
    }
}

impl InputView {
    /// Build an `InputView` from a JS props object.
    pub fn from_js(props: &JsObject, ctx: &mut Context) -> Self {
        let mut p = JsProps::new(props, ctx);
        InputView {
            width: p.val::<f64>("width"),
            height: p.val::<f64>("height"),
            controller: p.opaque::<TextEditingController>("controller"),
            controller_atom: p.readable("controller"),
            undo_controller: p.opaque::<UndoController>("undoController"),
            placeholder: p.val::<String>("placeholder"),
            color: p.val::<Color>("color"),
            placeholder_color: p.val::<Color>("placeholderColor"),
            cursor_color: p.val::<Color>("cursorColor"),
            font_size: p.val::<f64>("fontSize"),
            font_family: p.val::<String>("fontFamily"),
            multiline: p.val::<bool>("multiline"),
            on_context_menu: p.mutation::<ContextMenuEvent>("onContextMenu"),
            query_key: p.query_key("queryKey"),
        }
    }
}
