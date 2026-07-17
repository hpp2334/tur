//! Native platform integrations for tur.
//!
//! Currently provides [`NativeFontLoader`], a [`FontLoader`] that discovers
//! installed system fonts via fontique's platform backends (CoreText,
//! DirectWrite, fontconfig). Used by native embedders (e.g. integration
//! tests). The wasm embedder does not use this — it ships its own
//! bundled-font loader in `tur-wasm` instead.
//!
//! [`FontLoader`]: tur_engine::core::fonts::FontLoader

pub mod fonts;

pub use fonts::NativeFontLoader;
