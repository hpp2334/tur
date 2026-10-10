//! Platform-symbol glyph coverage in the embedded font chain.
//!
//! The browser (wasm) has no system fonts: the chain is exactly the bundled
//! Roboto + Roboto Mono faces, and neither maps U+2318 (PLACE OF INTEREST
//! SIGN) — so the playground status bar's "⌘S to run" renders a tofu box.
//! The engine's `core::fonts` therefore installs a built-in symbol fallback
//! slice ("Tur Symbols") as the last face on the generic families.
//!
//! These tests pin the SHIPPED wasm face set — bundled bytes only, no
//! system-font scan — so the coverage can't be silently satisfied by the dev
//! box's installed fonts. Red proof: with only the bundled faces, every
//! symbol below shapes to glyph 0 (.notdef) before the slice exists.

use std::sync::Arc;

use parley::{GenericFamily, LayoutContext, StyleProperty};
use tur_engine::core::fonts::{FontContext, FontLoader, FontManager, load_font_stack};

/// The exact faces `WasmFontLoader` ships (`libs/tur-wasm/src/fonts.rs`),
/// registered without any system fonts — the browser's reality.
struct BundledWasmFonts;

impl FontLoader for BundledWasmFonts {
    fn load_preset_fonts(&self, fcx: &mut FontContext) {
        let default: &[u8] = include_bytes!("../../../tur-wasm/fonts/Roboto-VF.ttf");
        let mono: &[u8] = include_bytes!("../../../tur-wasm/fonts/RobotoMono-VF.ttf");

        let families = fcx.collection.register_fonts(default.to_vec().into(), None);
        let roboto_ids: Vec<_> = families.into_iter().map(|(id, _)| id).collect();
        fcx.collection
            .set_generic_families(GenericFamily::SansSerif, roboto_ids.iter().copied());
        fcx.collection
            .set_generic_families(GenericFamily::Serif, roboto_ids.iter().copied());

        let mono_families = fcx.collection.register_fonts(mono.to_vec().into(), None);
        let mono_ids: Vec<_> = mono_families.into_iter().map(|(id, _)| id).collect();
        fcx.collection
            .set_generic_families(GenericFamily::Monospace, mono_ids.iter().copied());
    }
}

/// A `FontManager` over the shipped wasm face set only.
///
/// `system_fonts: false` drops fontique's system provider entirely — the
/// browser's reality. Without this, a native dev box's fontconfig fallback
/// (DejaVu & friends) would silently satisfy the coverage and the tests
/// would never see the shipped faces' tofu. The stack is built through the
/// engine's one fresh-stack sequence (`load_font_stack`) — the same
/// `TurRuntime::build` runs.
fn wasm_font_stack() -> FontManager {
    let mut fcx = FontContext {
        collection: parley::fontique::Collection::new(parley::fontique::CollectionOptions {
            system_fonts: false,
            ..Default::default()
        }),
        source_cache: parley::fontique::SourceCache::new(parley::fontique::SourceCacheOptions {
            shared: false,
        }),
    };
    load_font_stack(&mut fcx, &BundledWasmFonts);
    FontManager::from_context(fcx, Arc::new(BundledWasmFonts))
}

/// Shape `text` under `family` and return one `(char, glyph_id)` pair per
/// codepoint. Single-codepoint clusters only (the strings below contain no
/// multi-codepoint graphemes); tofu = glyph id 0 (.notdef).
fn shape_glyph_ids(mgr: &mut FontManager, family: GenericFamily, text: &str) -> Vec<(char, u32)> {
    let mut layout_cx = LayoutContext::<[u8; 4]>::new();
    let mut builder = layout_cx.ranged_builder(mgr.font_context(), text, 1.0, false);
    builder.push_default(StyleProperty::FontSize(11.0));
    builder.push_default(StyleProperty::from(family));
    let mut layout = builder.build(text);
    layout.break_all_lines(None);

    let mut out = Vec::new();
    for (offset, ch) in text.char_indices() {
        let cluster = parley::Cluster::from_byte_index(&layout, offset)
            .unwrap_or_else(|| panic!("no shaped cluster for {ch:?} at byte {offset}"));
        let gid = cluster.glyphs().next().map(|g| g.id).unwrap_or(0);
        out.push((ch, gid));
    }
    out
}

/// The playground status bar's exact hint string — every codepoint must
/// shape to a real glyph (⌘ from the fallback slice, the rest from Roboto).
#[test]
fn status_bar_command_glyph_shapes_non_tofu() {
    let mut mgr = wasm_font_stack();
    let ids = shape_glyph_ids(&mut mgr, GenericFamily::SansSerif, "\u{2318}S to run");
    assert_eq!(ids.len(), 9, "one entry per codepoint: {ids:?}");
    let tofu: Vec<char> = ids
        .iter()
        .filter(|&&(_, gid)| gid == 0)
        .map(|&(ch, _)| ch)
        .collect();
    assert!(
        tofu.is_empty(),
        "tofu glyph ids for {tofu:?} — U+2318 must shape to a real glyph"
    );
}

/// The small common platform-symbol set renders on the sans-serif stack.
#[test]
fn platform_symbol_set_shapes_non_tofu() {
    let mut mgr = wasm_font_stack();
    let symbols =
        "\u{2318}\u{2190}\u{2191}\u{2192}\u{2193}\u{21E5}\u{21E7}\u{2325}\u{232B}\u{23CE}";
    let ids = shape_glyph_ids(&mut mgr, GenericFamily::SansSerif, symbols);
    assert_eq!(ids.len(), symbols.chars().count());
    let missing: Vec<char> = ids
        .iter()
        .filter(|&&(_, gid)| gid == 0)
        .map(|&(ch, _)| ch)
        .collect();
    assert!(
        missing.is_empty(),
        "missing glyph coverage for {missing:?} in the embedded chain"
    );
}

/// The code editor's family (Monospace) falls back to the slice too, and
/// Latin text stays on the primary face (the slice is a last resort, not a
/// replacement).
#[test]
fn monospace_falls_back_while_latin_stays_on_primary() {
    let mut mgr = wasm_font_stack();
    let ids = shape_glyph_ids(&mut mgr, GenericFamily::Monospace, "\u{2318}fn main()");
    let tofu: Vec<char> = ids
        .iter()
        .filter(|&&(_, gid)| gid == 0)
        .map(|&(ch, _)| ch)
        .collect();
    assert!(
        tofu.is_empty(),
        "tofu glyph ids for {tofu:?} — monospace must reach the symbol slice"
    );
}
