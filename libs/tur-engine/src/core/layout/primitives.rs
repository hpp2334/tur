use num_derive::FromPrimitive;
use std::fmt;
use std::ops::{Add, Sub};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Size {
    pub width: f64,
    pub height: f64,
}

impl Size {
    pub const ZERO: Size = Size {
        width: 0.0,
        height: 0.0,
    };
    pub const INFINITE: Size = Size {
        width: f64::INFINITY,
        height: f64::INFINITY,
    };

    pub const fn new(width: f64, height: f64) -> Self {
        Size { width, height }
    }
}

impl Add for Size {
    type Output = Size;
    fn add(self, other: Size) -> Size {
        Size::new(self.width + other.width, self.height + other.height)
    }
}

impl Sub for Size {
    type Output = Size;
    fn sub(self, other: Size) -> Size {
        Size::new(self.width - other.width, self.height - other.height)
    }
}

impl fmt::Display for Size {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Size({:.1}, {:.1})", self.width, self.height)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Offset {
    pub x: f64,
    pub y: f64,
}

impl Offset {
    pub const ZERO: Offset = Offset { x: 0.0, y: 0.0 };

    pub const fn new(x: f64, y: f64) -> Self {
        Offset { x, y }
    }
}

impl Add for Offset {
    type Output = Offset;
    fn add(self, other: Offset) -> Offset {
        Offset::new(self.x + other.x, self.y + other.y)
    }
}

impl fmt::Display for Offset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Offset({:.1}, {:.1})", self.x, self.y)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EdgeInsets {
    pub left: f64,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
}

impl EdgeInsets {
    pub const ZERO: EdgeInsets = EdgeInsets {
        left: 0.0,
        top: 0.0,
        right: 0.0,
        bottom: 0.0,
    };

    pub const fn all(v: f64) -> Self {
        EdgeInsets {
            left: v,
            top: v,
            right: v,
            bottom: v,
        }
    }

    pub const fn symmetric(horizontal: f64, vertical: f64) -> Self {
        EdgeInsets {
            left: horizontal,
            top: vertical,
            right: horizontal,
            bottom: vertical,
        }
    }

    pub const fn only(left: f64, top: f64, right: f64, bottom: f64) -> Self {
        EdgeInsets {
            left,
            top,
            right,
            bottom,
        }
    }

    pub fn horizontal(&self) -> f64 {
        self.left + self.right
    }

    pub fn vertical(&self) -> f64 {
        self.top + self.bottom
    }

    pub fn inflate_size(&self, size: Size) -> Size {
        Size::new(
            size.width + self.horizontal(),
            size.height + self.vertical(),
        )
    }

    pub fn deflate_size(&self, size: Size) -> Size {
        Size::new(
            (size.width - self.horizontal()).max(0.0),
            (size.height - self.vertical()).max(0.0),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Constraints {
    pub min_width: f64,
    pub max_width: f64,
    pub min_height: f64,
    pub max_height: f64,
}

impl Constraints {
    pub const NONE: Constraints = Constraints {
        min_width: 0.0,
        max_width: f64::INFINITY,
        min_height: 0.0,
        max_height: f64::INFINITY,
    };

    pub fn tight(size: Size) -> Self {
        Constraints {
            min_width: size.width,
            max_width: size.width,
            min_height: size.height,
            max_height: size.height,
        }
    }

    pub fn loose(size: Size) -> Self {
        Constraints {
            min_width: 0.0,
            max_width: size.width,
            min_height: 0.0,
            max_height: size.height,
        }
    }

    pub fn constrain(&self, size: Size) -> Size {
        Size::new(
            size.width.clamp(self.min_width, self.max_width),
            size.height.clamp(self.min_height, self.max_height),
        )
    }

    pub fn deflate(&self, insets: EdgeInsets) -> Constraints {
        Constraints {
            min_width: (self.min_width - insets.horizontal()).max(0.0),
            max_width: (self.max_width - insets.horizontal()).max(0.0),
            min_height: (self.min_height - insets.vertical()).max(0.0),
            max_height: (self.max_height - insets.vertical()).max(0.0),
        }
    }

    pub fn is_tight(&self) -> bool {
        self.min_width >= self.max_width && self.min_height >= self.max_height
    }

    pub fn has_bounded_width(&self) -> bool {
        self.max_width < f64::INFINITY
    }

    pub fn has_bounded_height(&self) -> bool {
        self.max_height < f64::INFINITY
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, FromPrimitive)]
pub enum MainAxisAlignment {
    Start,
    Center,
    End,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, FromPrimitive)]
pub enum CrossAxisAlignment {
    Start,
    Center,
    End,
    Stretch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, FromPrimitive, Default)]
pub enum MainAxisSize {
    #[default]
    Max,
    Min,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, FromPrimitive)]
pub enum FlexFit {
    Tight,
    Loose,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, FromPrimitive)]
pub enum StackFit {
    Loose,
    Expand,
    Passthrough,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, FromPrimitive)]
pub enum FlexDirection {
    Vertical,
    Horizontal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, FromPrimitive)]
pub enum Axis {
    Vertical,
    Horizontal,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Geometry {
    Rect(Size),
    RoundedRect { size: Size, radius: f64 },
    Circle { radius: f64 },
}

impl Axis {
    pub fn main(&self, size: Size) -> f64 {
        match self {
            Axis::Horizontal => size.width,
            Axis::Vertical => size.height,
        }
    }

    pub fn cross(&self, size: Size) -> f64 {
        match self {
            Axis::Horizontal => size.height,
            Axis::Vertical => size.width,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ComputedLayout {
    pub size: Size,
    pub offset: Offset,
}

impl ComputedLayout {
    pub const ZERO: ComputedLayout = ComputedLayout {
        size: Size::ZERO,
        offset: Offset::ZERO,
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, FromPrimitive, Default)]
pub enum BoxFit {
    Fill,
    #[default]
    Contain,
    Cover,
    FitWidth,
    FitHeight,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, FromPrimitive, Default)]
pub enum BorderPosition {
    #[default]
    Inside,
    Center,
    Outside,
}

/// How a `Container` clips its child subtree to its decoration shape.
/// Mirrors Flutter's `Clip` enum. The default is `None` (no clipping),
/// matching Flutter's `Container.clipBehavior` default. When non-`None`,
/// children are clipped to the container's border geometry (rounded rect
/// when `borderRadius > 0`, else the plain rect).
///
/// Note: the vello backend implements every non-`None` mode as a single
/// anti-aliased clip layer, so `HardEdge` / `AntiAlias` /
/// `AntiAliasWithSaveLayer` are visually identical here — the variants
/// exist for API parity with Flutter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, FromPrimitive, Default)]
pub enum ClipBehavior {
    #[default]
    None,
    HardEdge,
    AntiAlias,
    AntiAliasWithSaveLayer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, FromPrimitive, Default)]
pub enum Alignment {
    #[default]
    TopLeft,
    TopCenter,
    TopRight,
    CenterLeft,
    Center,
    CenterRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
}

impl Alignment {
    pub fn align_offset(&self, container: Size, child: Size) -> Offset {
        let dw = (container.width - child.width).max(0.0);
        let dh = (container.height - child.height).max(0.0);
        let (fx, fy) = match self {
            Alignment::TopLeft => (0.0, 0.0),
            Alignment::TopCenter => (0.5, 0.0),
            Alignment::TopRight => (1.0, 0.0),
            Alignment::CenterLeft => (0.0, 0.5),
            Alignment::Center => (0.5, 0.5),
            Alignment::CenterRight => (1.0, 0.5),
            Alignment::BottomLeft => (0.0, 1.0),
            Alignment::BottomCenter => (0.5, 1.0),
            Alignment::BottomRight => (1.0, 1.0),
        };
        Offset::new(dw * fx, dh * fy)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, FromPrimitive, Default)]
pub enum HitTestBehavior {
    #[default]
    Opaque,
    Translucent,
}

/// Mouse button that produced a pointer event. Mirrors the DOM
/// `MouseEvent.button` value (0 = primary/left, 1 = middle, 2 = secondary/
/// right). Used by `PointerInput::PointerDown` / `PointerUp` so handlers
/// can distinguish left and right clicks without inspecting raw DOM events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MouseButton {
    #[default]
    Left,
    Middle,
    Right,
    Other(u16),
}

impl MouseButton {
    /// Build a `MouseButton` from a DOM `MouseEvent.button` value.
    pub fn from_dom(button: u16) -> Self {
        match button {
            0 => MouseButton::Left,
            1 => MouseButton::Middle,
            2 => MouseButton::Right,
            other => MouseButton::Other(other),
        }
    }
}
