use crate::core::render::brush::Color;

/// One styled run of the controller's span tree.
///
/// `PartialEq` compares **rendered content** (text + every style field) so
/// the controller can skip a content-revision bump for no-op re-highlights
/// (the same tokens re-set with identical styles).
#[derive(Clone, PartialEq)]
pub struct SpanData {
    pub text: String,
    /// CSS-style numeric font weight (100–1000). `None` inherits the
    /// element's default (which is itself 400 — `FontWeight::NORMAL` — when
    /// unset). Parley resolves this to a face for static fonts or to a
    /// variation-axis coordinate for variable fonts.
    pub(crate) weight: Option<f64>,
    pub(crate) italic: bool,
    pub(crate) underline: bool,
    pub(crate) font_size: Option<f64>,
    pub(crate) color: Option<Color>,
}

impl SpanData {
    /// The span's pinned color, if any (the highlighting merge probe —
    /// adjacent same-colored runs coalesce into one).
    pub fn color(&self) -> Option<Color> {
        self.color
    }

    /// A single-color text run — the syntax-highlighting payload shape.
    /// Every style field except the color inherits the element's defaults
    /// (the fields stay crate-private; styled runs are minted engine-side).
    pub fn colored(text: impl Into<String>, color: Color) -> Self {
        Self {
            text: text.into(),
            weight: None,
            italic: false,
            underline: false,
            font_size: None,
            color: Some(color),
        }
    }
}
