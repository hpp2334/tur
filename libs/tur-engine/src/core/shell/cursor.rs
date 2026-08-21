/// The set of OS cursor styles, mirroring the standard CSS cursor
/// keywords. The embedder maps a `Cursor` back to its keyword via
/// [`Cursor::as_str`] (e.g. for the web canvas `style.cursor`, or a
/// host-native cursor icon).
///
/// Unrecognized keyword strings fail `FromJs` decode (see the `FromJs`
/// impl in tur-engine); callers that want to tolerate them must opt in.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Cursor {
    Auto,
    #[default]
    Default,
    None,
    ContextMenu,
    Help,
    Pointer,
    Progress,
    Wait,
    Cell,
    Crosshair,
    Text,
    VerticalText,
    Alias,
    Copy,
    Move,
    NoDrop,
    NotAllowed,
    Grab,
    Grabbing,
    EResize,
    NResize,
    NeResize,
    NwResize,
    SResize,
    SeResize,
    SwResize,
    WResize,
    EwResize,
    NsResize,
    NeswResize,
    NwseResize,
    ColResize,
    RowResize,
    AllScroll,
    ZoomIn,
    ZoomOut,
}

impl Cursor {
    /// The standard CSS cursor keyword for this variant (e.g. `"col-resize"`).
    pub fn as_str(self) -> &'static str {
        match self {
            Cursor::Auto => "auto",
            Cursor::Default => "default",
            Cursor::None => "none",
            Cursor::ContextMenu => "context-menu",
            Cursor::Help => "help",
            Cursor::Pointer => "pointer",
            Cursor::Progress => "progress",
            Cursor::Wait => "wait",
            Cursor::Cell => "cell",
            Cursor::Crosshair => "crosshair",
            Cursor::Text => "text",
            Cursor::VerticalText => "vertical-text",
            Cursor::Alias => "alias",
            Cursor::Copy => "copy",
            Cursor::Move => "move",
            Cursor::NoDrop => "no-drop",
            Cursor::NotAllowed => "not-allowed",
            Cursor::Grab => "grab",
            Cursor::Grabbing => "grabbing",
            Cursor::EResize => "e-resize",
            Cursor::NResize => "n-resize",
            Cursor::NeResize => "ne-resize",
            Cursor::NwResize => "nw-resize",
            Cursor::SResize => "s-resize",
            Cursor::SeResize => "se-resize",
            Cursor::SwResize => "sw-resize",
            Cursor::WResize => "w-resize",
            Cursor::EwResize => "ew-resize",
            Cursor::NsResize => "ns-resize",
            Cursor::NeswResize => "nesw-resize",
            Cursor::NwseResize => "nwse-resize",
            Cursor::ColResize => "col-resize",
            Cursor::RowResize => "row-resize",
            Cursor::AllScroll => "all-scroll",
            Cursor::ZoomIn => "zoom-in",
            Cursor::ZoomOut => "zoom-out",
        }
    }

    /// Parse a CSS cursor keyword into a `Cursor`. Returns `None` for an
    /// unrecognized string; callers fall back to [`Cursor::Default`].
    pub fn from_keyword(s: &str) -> Option<Cursor> {
        Some(match s {
            "auto" => Cursor::Auto,
            "default" => Cursor::Default,
            "none" => Cursor::None,
            "context-menu" => Cursor::ContextMenu,
            "help" => Cursor::Help,
            "pointer" => Cursor::Pointer,
            "progress" => Cursor::Progress,
            "wait" => Cursor::Wait,
            "cell" => Cursor::Cell,
            "crosshair" => Cursor::Crosshair,
            "text" => Cursor::Text,
            "vertical-text" => Cursor::VerticalText,
            "alias" => Cursor::Alias,
            "copy" => Cursor::Copy,
            "move" => Cursor::Move,
            "no-drop" => Cursor::NoDrop,
            "not-allowed" => Cursor::NotAllowed,
            "grab" => Cursor::Grab,
            "grabbing" => Cursor::Grabbing,
            "e-resize" => Cursor::EResize,
            "n-resize" => Cursor::NResize,
            "ne-resize" => Cursor::NeResize,
            "nw-resize" => Cursor::NwResize,
            "s-resize" => Cursor::SResize,
            "se-resize" => Cursor::SeResize,
            "sw-resize" => Cursor::SwResize,
            "w-resize" => Cursor::WResize,
            "ew-resize" => Cursor::EwResize,
            "ns-resize" => Cursor::NsResize,
            "nesw-resize" => Cursor::NeswResize,
            "nwse-resize" => Cursor::NwseResize,
            "col-resize" => Cursor::ColResize,
            "row-resize" => Cursor::RowResize,
            "all-scroll" => Cursor::AllScroll,
            "zoom-in" => Cursor::ZoomIn,
            "zoom-out" => Cursor::ZoomOut,
            _ => return None,
        })
    }
}
