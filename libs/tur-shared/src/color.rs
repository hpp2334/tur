use std::fmt;
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, TS)]
#[ts(export, export_to = "generated/")]
pub struct RGB {
    pub r: u8,
    pub g: u8,
    pub b: u8,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, TS)]
#[ts(export, export_to = "generated/")]
pub struct RGBA {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, TS)]
#[ts(export, export_to = "generated/")]
pub enum Color {
    RGB(RGB),
    RGBA(RGBA),
}

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Color::RGB(RGB::new(r, g, b))
    }

    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Color::RGBA(RGBA::new(r, g, b, a))
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
