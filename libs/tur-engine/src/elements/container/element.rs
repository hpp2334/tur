use boa_engine::{Context, JsString, JsValue};
use num_traits::FromPrimitive;
use tur_shared::{BorderPosition, Brush, Color};

use crate::core::bridge::color::extract_brush;
use crate::core::elements::ElementOnUpdate;
use crate::core::elements::ElementTrace;

#[derive(Clone, Default)]
pub struct ContainerElement {
    pub(crate) width: Option<f64>,
    pub(crate) height: Option<f64>,
    pub(crate) padding: Option<f64>,
    pub(crate) color: Option<Brush>,
    pub(crate) border_color: Option<Color>,
    pub(crate) border_width: Option<f64>,
    pub(crate) border_radius: Option<f64>,
    pub(crate) border_position: BorderPosition,
    pub(crate) shadow_color: Option<Color>,
    pub(crate) shadow_offset: Option<(f64, f64)>,
    pub(crate) shadow_blur: Option<f64>,
}

impl ContainerElement {
    pub fn new() -> Self {
        ContainerElement {
            width: None,
            height: None,
            padding: None,
            color: None,
            border_color: None,
            border_width: None,
            border_radius: None,
            border_position: BorderPosition::default(),
            shadow_color: None,
            shadow_offset: None,
            shadow_blur: None,
        }
    }

    pub fn border_color(&self) -> Option<&Color> {
        self.border_color.as_ref()
    }

    pub fn border_width(&self) -> Option<f64> {
        self.border_width
    }

    pub fn border_radius(&self) -> Option<f64> {
        self.border_radius
    }

    pub fn border_position(&self) -> BorderPosition {
        self.border_position
    }

    pub fn width(&self) -> Option<f64> {
        self.width
    }

    pub fn height(&self) -> Option<f64> {
        self.height
    }

    pub fn padding(&self) -> Option<f64> {
        self.padding
    }

    pub fn shadow_color(&self) -> Option<&Color> {
        self.shadow_color.as_ref()
    }

    pub fn shadow_offset(&self) -> Option<(f64, f64)> {
        self.shadow_offset
    }

    pub fn shadow_blur(&self) -> Option<f64> {
        self.shadow_blur
    }
}

impl ElementTrace for ContainerElement {
    fn trace_label(&self) -> String {
        let mut parts = Vec::new();
        if let Some(w) = self.width {
            parts.push(format!("width={w}"));
        }
        if let Some(h) = self.height {
            parts.push(format!("height={h}"));
        }
        if let Some(p) = self.padding {
            parts.push(format!("padding={p}"));
        }
        if let Some(ref b) = self.color {
            match b {
                Brush::SolidColor(c) => parts.push(format!("color={c}")),
                Brush::LinearGradient { .. } => parts.push("color=linearGradient".into()),
            }
        }
        if let Some(c) = self.border_color {
            parts.push(format!("borderColor={c}"));
        }
        if let Some(w) = self.border_width {
            parts.push(format!("borderWidth={w}"));
        }
        if let Some(r) = self.border_radius {
            parts.push(format!("borderRadius={r}"));
        }
        parts.push(format!("borderPosition={:?}", self.border_position));
        if let Some(c) = self.shadow_color {
            parts.push(format!("shadowColor={c}"));
        }
        if let Some((x, y)) = self.shadow_offset {
            parts.push(format!("shadowOffset=({x},{y})"));
        }
        if let Some(b) = self.shadow_blur {
            parts.push(format!("shadowBlur={b}"));
        }
        parts.join(" ")
    }
}

fn extract_offset_array(value: &JsValue, ctx: &mut Context) -> Option<(f64, f64)> {
    let obj = value.as_object()?;
    let arr = boa_engine::object::builtins::JsArray::from_object(obj.clone()).ok()?;
    let x = arr.at(0, ctx).ok()?.as_number()?;
    let y = arr.at(1, ctx).ok()?.as_number()?;
    Some((x, y))
}

impl ElementOnUpdate for ContainerElement {
    fn set_prop(&mut self, _ctx: &mut Context, key: &JsString, value: &JsValue) {
        if *key == "width" {
            self.width = value.as_number();
        } else if *key == "height" {
            self.height = value.as_number();
        } else if *key == "padding" {
            self.padding = value.as_number();
        } else if *key == "color" {
            self.color = extract_brush(value, _ctx);
        } else if *key == "borderColor" {
            self.border_color = crate::core::bridge::color::extract_color(value, _ctx);
        } else if *key == "borderWidth" {
            self.border_width = value.as_number();
        } else if *key == "borderRadius" {
            self.border_radius = value.as_number();
        } else if *key == "borderPosition" {
            if let Some(n) = value.as_number() {
                self.border_position =
                    BorderPosition::from_u8(n as u8).unwrap_or_default();
            }
        } else if *key == "shadowColor" {
            self.shadow_color = crate::core::bridge::color::extract_color(value, _ctx);
        } else if *key == "shadowOffset" {
            self.shadow_offset = extract_offset_array(value, _ctx);
        } else if *key == "shadowBlur" {
            self.shadow_blur = value.as_number();
        }
    }
}
