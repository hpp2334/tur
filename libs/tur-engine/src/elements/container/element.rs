use boa_engine::{Context, JsString, JsValue};
use num_traits::FromPrimitive;
use tur_shared::{Alignment, AnimatableValue, BorderPosition, Brush, Color, Size};

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
    pub(crate) alignment: Option<Alignment>,
    pub(crate) computed_size: Option<Size>,
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
            alignment: None,
            computed_size: None,
        }
    }

    pub fn border_color(&self) -> Option<&Color> {
        self.border_color.as_ref()
    }

    pub fn color(&self) -> Option<&Brush> {
        self.color.as_ref()
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
        if let Some(ref a) = self.alignment {
            parts.push(format!("alignment={a:?}"));
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
        } else if *key == "alignment" {
            if let Some(n) = value.as_number() {
                self.alignment = Alignment::from_i32(n as i32);
            }
        }
    }

    fn reset_prop(&mut self, key: &JsString) {
        match key.to_std_string_escaped().as_str() {
            "width" => self.width = None,
            "height" => self.height = None,
            "padding" => self.padding = None,
            "color" => self.color = None,
            "borderColor" => self.border_color = None,
            "borderWidth" => self.border_width = None,
            "borderRadius" => self.border_radius = None,
            "borderPosition" => self.border_position = BorderPosition::default(),
            "shadowColor" => self.shadow_color = None,
            "shadowOffset" => self.shadow_offset = None,
            "shadowBlur" => self.shadow_blur = None,
            "alignment" => self.alignment = None,
            _ => {}
        }
    }

    fn apply_animated(&mut self, key: &str, value: AnimatableValue) {
        match key {
            "width" => {
                if let AnimatableValue::Float(v) = value {
                    self.width = Some(v)
                }
            }
            "height" => {
                if let AnimatableValue::Float(v) = value {
                    self.height = Some(v)
                }
            }
            "padding" => {
                if let AnimatableValue::Float(v) = value {
                    self.padding = Some(v)
                }
            }
            "borderWidth" => {
                if let AnimatableValue::Float(v) = value {
                    self.border_width = Some(v)
                }
            }
            "borderRadius" => {
                if let AnimatableValue::Float(v) = value {
                    self.border_radius = Some(v)
                }
            }
            "shadowBlur" => {
                if let AnimatableValue::Float(v) = value {
                    self.shadow_blur = Some(v)
                }
            }
            "color" => {
                if let AnimatableValue::Color(c) = value {
                    self.color = Some(Brush::SolidColor(c))
                }
            }
            "borderColor" => {
                if let AnimatableValue::Color(c) = value {
                    self.border_color = Some(c)
                }
            }
            "shadowColor" => {
                if let AnimatableValue::Color(c) = value {
                    self.shadow_color = Some(c)
                }
            }
            _ => {}
        }
    }

    fn get_animatable(&self, key: &str) -> Option<AnimatableValue> {
        match key {
            "width" => self.width.map(AnimatableValue::Float),
            "height" => self.height.map(AnimatableValue::Float),
            "padding" => self.padding.map(AnimatableValue::Float),
            "borderWidth" => self.border_width.map(AnimatableValue::Float),
            "borderRadius" => self.border_radius.map(AnimatableValue::Float),
            "shadowBlur" => self.shadow_blur.map(AnimatableValue::Float),
            "color" => self.color.as_ref().and_then(|b| match b {
                Brush::SolidColor(c) => Some(AnimatableValue::Color(*c)),
                _ => None,
            }),
            "borderColor" => self.border_color.map(AnimatableValue::Color),
            "shadowColor" => self.shadow_color.map(AnimatableValue::Color),
            _ => None,
        }
    }
}
