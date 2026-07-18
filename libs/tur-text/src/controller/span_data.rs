use tur_engine::core::render::Color;

use tur_engine::core::js_value::FromJs;

#[derive(Clone)]
pub struct SpanData {
    pub text: String,
    pub(crate) bold: bool,
    pub(crate) italic: bool,
    pub(crate) underline: bool,
    pub(crate) font_size: Option<f64>,
    pub(crate) color: Option<Color>,
}

pub fn extract_spans_from_js(
    value: &boa_engine::JsValue,
    context: &mut boa_engine::Context,
) -> Vec<SpanData> {
    let Some(obj) = value.as_object() else {
        return Vec::new();
    };
    let Ok(arr) = boa_engine::object::builtins::JsArray::from_object(obj.clone()) else {
        return Vec::new();
    };
    let len = match arr.length(context) {
        Ok(l) => l as usize,
        Err(_) => return Vec::new(),
    };

    let mut spans = Vec::with_capacity(len);
    for i in 0..len {
        let Ok(span_val) = arr.at(i as i64, context) else {
            continue;
        };
        let Some(span_obj) = span_val.as_object() else {
            continue;
        };

        let content = span_obj
            .get(boa_engine::js_string!("content"), context)
            .ok()
            .and_then(|v| v.as_string().map(|s| s.to_std_string_escaped()))
            .unwrap_or_default();

        let bold = span_obj
            .get(boa_engine::js_string!("bold"), context)
            .ok()
            .and_then(|v| v.as_boolean())
            .unwrap_or(false);

        let italic = span_obj
            .get(boa_engine::js_string!("italic"), context)
            .ok()
            .and_then(|v| v.as_boolean())
            .unwrap_or(false);

        let underline = span_obj
            .get(boa_engine::js_string!("underline"), context)
            .ok()
            .and_then(|v| v.as_boolean())
            .unwrap_or(false);

        let font_size = span_obj
            .get(boa_engine::js_string!("fontSize"), context)
            .ok()
            .and_then(|v| v.as_number());

        let color = span_obj
            .get(boa_engine::js_string!("color"), context)
            .ok()
            .and_then(|v| Color::from_js(&v).ok());

        spans.push(SpanData {
            text: content,
            bold,
            italic,
            underline,
            font_size,
            color,
        });
    }
    spans
}
