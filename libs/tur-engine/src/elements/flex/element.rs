use boa_engine::{Context, JsString, JsValue};
use num_traits::FromPrimitive;
use tur_shared::{Axis, Constraints, CrossAxisAlignment, MainAxisAlignment, Size};

use crate::core::elements::ElementOnUpdate;

pub(crate) struct ChildData {
    pub id: crate::core::element::ElementNodeId,
    pub size: Size,
    pub is_flex: bool,
}

pub struct FlexElement {
    pub(crate) direction: Axis,
    pub(crate) main_alignment: MainAxisAlignment,
    pub(crate) cross_alignment: CrossAxisAlignment,
    pub(crate) child_data: Vec<ChildData>,
    pub(crate) constraints: Option<Constraints>,
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
            child_data: Vec::new(),
            constraints: None,
        }
    }
}

impl ElementOnUpdate for FlexElement {
    fn set_prop(&mut self, _ctx: &mut Context, key: &JsString, value: &JsValue) {
        if *key == "direction" {
            if let Some(n) = value.as_number() {
                self.direction = Axis::from_i32(n as i32).unwrap_or(self.direction);
            }
        } else if *key == "mainAlignment" {
            if let Some(n) = value.as_number() {
                self.main_alignment =
                    MainAxisAlignment::from_i32(n as i32).unwrap_or(self.main_alignment);
            }
        } else if *key == "crossAlignment" {
            if let Some(n) = value.as_number() {
                self.cross_alignment =
                    CrossAxisAlignment::from_i32(n as i32).unwrap_or(self.cross_alignment);
            }
        }
    }
}
