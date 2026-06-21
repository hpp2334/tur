/// Element-specific dev-tool / debug introspection.
///
/// `trace_label` produces a short human-readable hint (e.g. a text preview
/// or `"width=100 height=50"`). `trace_props` and `trace_layout_extra`
/// feed the structured `turDevTool` output: `trace_props` describes
/// configuration / state, while `trace_layout_extra` describes
/// element-specific computed-layout metadata (e.g. an editable text's
/// line count) that does not apply to every element.
#[derive(Debug, Clone)]
pub enum TraceValue {
    Str(String),
    Num(f64),
    Bool(bool),
    Null,
}

pub trait ElementTrace {
    fn trace_label(&self) -> String {
        String::new()
    }

    fn trace_props(&self) -> Vec<(&'static str, TraceValue)> {
        Vec::new()
    }

    fn trace_layout_extra(&self) -> Vec<(&'static str, TraceValue)> {
        Vec::new()
    }
}
