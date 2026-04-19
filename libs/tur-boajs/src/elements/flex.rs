use crate::impl_dyn_element;
use boa_engine::{Context, JsString, JsValue};
use tur_element_tree::Element;
use tur_element_tree::ElementKind;
use tur_render_tree::FlexRenderObject;
use tur_render_tree::{Axis, CrossAxisAlignment, MainAxisAlignment};

#[derive(Clone)]
pub struct FlexElement {
    direction: Axis,
    main_alignment: MainAxisAlignment,
    cross_alignment: CrossAxisAlignment,
}

impl Default for FlexElement {
    fn default() -> Self {
        Self::new()
    }
}

impl FlexElement {
    pub fn new() -> Self {
        FlexElement {
            direction: Axis::Vertical,
            main_alignment: MainAxisAlignment::Start,
            cross_alignment: CrossAxisAlignment::Center,
        }
    }
}

impl Element for FlexElement {
    type TypedRenderObject = FlexRenderObject;

    fn to_render_object(&self) -> FlexRenderObject {
        FlexRenderObject::new(self.direction, self.main_alignment, self.cross_alignment)
    }

    fn kind(&self) -> ElementKind {
        ElementKind::new("tur_flex")
    }

    fn name(&self) -> &'static str {
        "tur_flex"
    }
}

impl_dyn_element!(FlexElement);

impl crate::elements::BoaElement for FlexElement {
    fn set_prop(&mut self, _ctx: &mut Context, key: &JsString, value: &JsValue) {
        let key_str = key.to_std_string_escaped();
        match key_str.as_str() {
            "direction" => {
                if let Some(s) = value.as_string() {
                    self.direction = match s.to_std_string_escaped().as_str() {
                        "Vertical" => Axis::Vertical,
                        "Horizontal" => Axis::Horizontal,
                        _ => return,
                    };
                } else if let Some(n) = value.as_number() {
                    self.direction = match n as i32 {
                        0 => Axis::Vertical,
                        1 => Axis::Horizontal,
                        _ => return,
                    };
                }
            }
            "mainAlignment" => {
                if let Some(s) = value.as_string() {
                    self.main_alignment = match s.to_std_string_escaped().as_str() {
                        "start" => MainAxisAlignment::Start,
                        "center" => MainAxisAlignment::Center,
                        "end" => MainAxisAlignment::End,
                        "space-between" => MainAxisAlignment::SpaceBetween,
                        "space-around" => MainAxisAlignment::SpaceAround,
                        "space-evenly" => MainAxisAlignment::SpaceEvenly,
                        _ => return,
                    };
                } else if let Some(n) = value.as_number() {
                    self.main_alignment = match n as i32 {
                        0 => MainAxisAlignment::Start,
                        1 => MainAxisAlignment::Center,
                        2 => MainAxisAlignment::End,
                        3 => MainAxisAlignment::SpaceBetween,
                        4 => MainAxisAlignment::SpaceAround,
                        5 => MainAxisAlignment::SpaceEvenly,
                        _ => return,
                    };
                }
            }
            "crossAlignment" => {
                if let Some(s) = value.as_string() {
                    self.cross_alignment = match s.to_std_string_escaped().as_str() {
                        "start" => CrossAxisAlignment::Start,
                        "center" => CrossAxisAlignment::Center,
                        "end" => CrossAxisAlignment::End,
                        "stretch" => CrossAxisAlignment::Stretch,
                        _ => return,
                    };
                } else if let Some(n) = value.as_number() {
                    self.cross_alignment = match n as i32 {
                        0 => CrossAxisAlignment::Start,
                        1 => CrossAxisAlignment::Center,
                        2 => CrossAxisAlignment::End,
                        3 => CrossAxisAlignment::Stretch,
                        _ => return,
                    };
                }
            }
            _ => {}
        }
    }
}
