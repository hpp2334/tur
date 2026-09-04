use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RGB {
    r: u8,
    g: u8,
    b: u8,
}

impl RGB {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        RGB { r, g, b }
    }
}

impl fmt::Display for RGB {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RGBA {
    r: u8,
    g: u8,
    b: u8,
    a: u8,
}

impl RGBA {
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        RGBA { r, g, b, a }
    }
}

impl fmt::Display for RGBA {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "#{:02x}{:02x}{:02x}{:02x}",
            self.r, self.g, self.b, self.a
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
    RGB(RGB),
    RGBA(RGBA),
}

impl Color {
    /// Opaque white — the renderers' default base (background) color.
    pub const WHITE: Color = Color::RGB(RGB::new(255, 255, 255));

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Color::RGB(RGB::new(r, g, b))
    }

    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Color::RGBA(RGBA::new(r, g, b, a))
    }

    pub const fn r(&self) -> u8 {
        match self {
            Color::RGB(c) => c.r,
            Color::RGBA(c) => c.r,
        }
    }

    pub const fn g(&self) -> u8 {
        match self {
            Color::RGB(c) => c.g,
            Color::RGBA(c) => c.g,
        }
    }

    pub const fn b(&self) -> u8 {
        match self {
            Color::RGB(c) => c.b,
            Color::RGBA(c) => c.b,
        }
    }

    pub const fn a(&self) -> u8 {
        match self {
            Color::RGB(_) => 255,
            Color::RGBA(c) => c.a,
        }
    }

    /// Linear interpolation between two colors at parameter `t` in `[0, 1]`,
    /// component-wise in u8 space (each channel rounded to nearest). Matches
    /// Flutter's `Color.lerp` semantics: `t=0` returns `a` exactly, `t=1`
    /// returns `b` exactly (preserving `RGB` vs `RGBA` representation); only
    /// intermediate values interpolate. Intermediate results are always
    /// `RGBA` so an interpolated alpha is preserved even when both inputs
    /// are `RGB` (treated as `a=255`).
    ///
    /// Values of `t` outside `[0, 1]` are clamped.
    pub fn lerp(a: Color, b: Color, t: f64) -> Color {
        let t = t.clamp(0.0, 1.0);
        if t == 0.0 {
            return a;
        }
        if t == 1.0 {
            return b;
        }
        let ch = |x: u8, y: u8| -> u8 {
            let v = x as f64 + (y as f64 - x as f64) * t;
            v.round().clamp(0.0, 255.0) as u8
        };
        Color::RGBA(RGBA::new(
            ch(a.r(), b.r()),
            ch(a.g(), b.g()),
            ch(a.b(), b.b()),
            ch(a.a(), b.a()),
        ))
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Color::RGB(rgb) => write!(f, "{rgb}"),
            Color::RGBA(rgba) => write!(f, "{rgba}"),
        }
    }
}

impl std::str::FromStr for Color {
    type Err = ColorParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let hex = s.trim_start_matches('#');
        match hex.len() {
            3 => {
                let r =
                    u8::from_str_radix(&hex[0..1].repeat(2), 16).map_err(|_| ColorParseError)?;
                let g =
                    u8::from_str_radix(&hex[1..2].repeat(2), 16).map_err(|_| ColorParseError)?;
                let b =
                    u8::from_str_radix(&hex[2..3].repeat(2), 16).map_err(|_| ColorParseError)?;
                Ok(Color::RGB(RGB::new(r, g, b)))
            }
            6 => {
                let r = u8::from_str_radix(&hex[0..2], 16).map_err(|_| ColorParseError)?;
                let g = u8::from_str_radix(&hex[2..4], 16).map_err(|_| ColorParseError)?;
                let b = u8::from_str_radix(&hex[4..6], 16).map_err(|_| ColorParseError)?;
                Ok(Color::RGB(RGB::new(r, g, b)))
            }
            8 => {
                let r = u8::from_str_radix(&hex[0..2], 16).map_err(|_| ColorParseError)?;
                let g = u8::from_str_radix(&hex[2..4], 16).map_err(|_| ColorParseError)?;
                let b = u8::from_str_radix(&hex[4..6], 16).map_err(|_| ColorParseError)?;
                let a = u8::from_str_radix(&hex[6..8], 16).map_err(|_| ColorParseError)?;
                Ok(Color::RGBA(RGBA::new(r, g, b, a)))
            }
            _ => Err(ColorParseError),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("invalid color format, expected #RGB, #RRGGBB, or #RRGGBBAA")]
pub struct ColorParseError;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GradientStop {
    pub offset: f32,
    pub color: Color,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Brush {
    SolidColor(Color),
    LinearGradient {
        start: (f64, f64),
        end: (f64, f64),
        stops: Vec<GradientStop>,
    },
}
