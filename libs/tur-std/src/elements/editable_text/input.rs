use std::rc::Rc;

use boa_engine::object::JsObject;
use boa_engine::Context;
use tur_shared::Color;

use tur_engine::core::element::NodeId;
use tur_engine::core::view::{ViewCx, View, Val};
use tur_engine::core::reactive::AnyReadable;
use tur_engine::elements::ContainerView;

use super::element::{prop_controller, prop_controller_atom, prop_mutation, prop_query_key, prop_undo_controller, prop_val, ContextMenuEvent, EditableTextView};

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
    on_context_menu: Option<tur_engine::core::edgy_event::EdgyMutation<ContextMenuEvent>>,
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
        InputView {
            width: prop_val::<f64>(props, "width", ctx),
            height: prop_val::<f64>(props, "height", ctx),
            controller: prop_controller(props, "controller", ctx),
            controller_atom: prop_controller_atom(props, "controller", ctx),
            undo_controller: prop_undo_controller(props, "undoController", ctx),
            placeholder: prop_val::<String>(props, "placeholder", ctx),
            color: prop_val::<Color>(props, "color", ctx),
            placeholder_color: prop_val::<Color>(props, "placeholderColor", ctx),
            cursor_color: prop_val::<Color>(props, "cursorColor", ctx),
            font_size: prop_val::<f64>(props, "fontSize", ctx),
            font_family: prop_val::<String>(props, "fontFamily", ctx),
            multiline: prop_val::<bool>(props, "multiline", ctx),
            on_context_menu: prop_mutation::<ContextMenuEvent>(props, "onContextMenu", ctx),
            query_key: prop_query_key(props, "queryKey", ctx),
        }
    }
}
