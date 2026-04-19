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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColorParseError;

impl fmt::Display for ColorParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid color format, expected #RGB, #RRGGBB, or #RRGGBBAA"
        )
    }
}

impl std::error::Error for ColorParseError {}
