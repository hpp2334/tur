use tur_element::ElementKind;
use tur_shared::ComputedLayout;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RenderNodeId(u64);

impl RenderNodeId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    pub fn as_u64(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone)]
pub struct RenderNode {
    pub id: RenderNodeId,
    pub kind: ElementKind,
    pub children: Vec<RenderNodeId>,
    pub computed_layout: ComputedLayout,
    pub text_content: Option<String>,
    pub font_size: Option<f64>,
    pub color: Option<String>,
    pub padding: Option<f64>,
}
