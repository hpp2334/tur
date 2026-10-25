//! The font resolver: Flutter-style `(fontFamily, codePoint)` resolution.
//!
//! `resolve_font_stack` builds the ordered stack — primary family BY NAME,
//! explicit fallbacks in order, then the engine default family (sans) — and
//! parley's shaper selects per codepoint across it. These tests pin that on
//! the shipped wasm face set (bundled bytes only, no system fonts — the
//! browser's reality): a monospace NAME must actually render monospaced
//! (uniform advances), an unknown name must fall through to the default, and
//! an explicit fallback list must be consulted in order.
//!
//! Discriminator: Roboto Mono gives every glyph the same advance (it is a
//! monospace face); Roboto does not. `advance('A') == advance('I')` under a
//! stack therefore proves the mono face served the run; a tofu glyph id (0)
//! proves the walk reached no covering face.

use std::sync::Arc;

use parley::{LayoutContext, StyleProperty};
use tur_engine::core::fonts::{
    FontContext, FontLoader, FontManager, load_font_stack, resolve_font_stack,
};

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
            .set_generic_families(parley::GenericFamily::SansSerif, roboto_ids.iter().copied());
        fcx.collection
            .set_generic_families(parley::GenericFamily::Serif, roboto_ids.iter().copied());

        let mono_families = fcx.collection.register_fonts(mono.to_vec().into(), None);
        let mono_ids: Vec<_> = mono_families.into_iter().map(|(id, _)| id).collect();
        fcx.collection
            .set_generic_families(parley::GenericFamily::Monospace, mono_ids.iter().copied());
    }
}

/// A `FontManager` over the shipped wasm face set only (`system_fonts:
/// false` — a native dev box's fontconfig fallback would silently satisfy
/// every assertion). Built through the engine's one fresh-stack sequence.
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
    let rail = Arc::new(tur_engine::core::fonts::FontRail::new(fcx.clone()));
    FontManager::from_context(fcx, rail)
}

/// Shape `text` under `resolve_font_stack(family, fallback)` and return one
/// `(char, glyph_id, advance)` per codepoint. Single-codepoint clusters only
/// (the strings below contain no multi-codepoint graphemes).
fn shape(
    mgr: &mut FontManager,
    family: Option<&str>,
    fallback: Option<&[String]>,
    text: &str,
) -> Vec<(char, u32, f32)> {
    let mut layout_cx = LayoutContext::<[u8; 4]>::new();
    let mut builder = layout_cx.ranged_builder(mgr.font_context(), text, 1.0, false);
    builder.push_default(StyleProperty::FontSize(11.0));
    builder.push_default(StyleProperty::FontFamily(resolve_font_stack(
        family, fallback,
    )));
    let mut layout = builder.build(text);
    layout.break_all_lines(None);

    text.char_indices()
        .map(|(offset, ch)| {
            let cluster = parley::Cluster::from_byte_index(&layout, offset)
                .unwrap_or_else(|| panic!("no shaped cluster for {ch:?} at byte {offset}"));
            let glyph = cluster.glyphs().next().unwrap_or_else(|| {
                panic!("no glyph for {ch:?} at byte {offset}");
            });
            (ch, glyph.id, glyph.advance)
        })
        .collect()
}

fn advances(shaped: &[(char, u32, f32)]) -> Vec<f32> {
    shaped.iter().map(|&(_, _, a)| a).collect()
}

fn tofu(shaped: &[(char, u32, f32)]) -> Vec<char> {
    shaped
        .iter()
        .filter(|&&(_, gid, _)| gid == 0)
        .map(|&(ch, _, _)| ch)
        .collect()
}

fn advances_eq(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.01
}

/// The `fontFamily` NAME resolves against registered families: "Roboto Mono"
/// (a real registered family on the wasm preset) must render monospaced —
/// uniform advances for every glyph. Red under the old resolver, which
/// collapsed every non-keyword string to the sans-serif generic.
#[test]
fn font_family_resolves_by_name() {
    let mut mgr = wasm_font_stack();
    let shaped = shape(&mut mgr, Some("Roboto Mono"), None, "AIl1.");
    assert!(
        tofu(&shaped).is_empty(),
        "tofu for {tofu:?} — 'Roboto Mono' must resolve by name",
        tofu = tofu(&shaped)
    );
    let adv = advances(&shaped);
    let uniform = adv.iter().all(|a| advances_eq(*a, adv[0]));
    assert!(
        uniform,
        "advances {adv:?} — 'Roboto Mono' must actually render monospaced"
    );
}

/// Sanity guard for the discriminator itself: the sans face gives 'A' and 'I'
/// DIFFERENT advances.
#[test]
fn sans_baseline_advances_differ() {
    let mut mgr = wasm_font_stack();
    let shaped = shape(&mut mgr, Some("Roboto"), None, "AI");
    assert!(tofu(&shaped).is_empty(), "tofu for the sans baseline");
    let adv = advances(&shaped);
    assert!(
        !advances_eq(adv[0], adv[1]),
        "advances {adv:?} — Roboto must NOT be uniform; the discriminator is broken"
    );
}

/// `(fontFamily, codePoint)`: under the "Roboto Mono" stack, Latin stays on
/// the mono face (uniform advances) while ⌘ — uncovered by Roboto Mono —
/// shapes from the engine default tail's last-resort symbol face. Every
/// codepoint non-tofu.
#[test]
fn per_codepoint_fallback_mixes_faces() {
    let mut mgr = wasm_font_stack();
    let shaped = shape(&mut mgr, Some("Roboto Mono"), None, "AI\u{2318}");
    assert_eq!(shaped.len(), 3);
    let missing = tofu(&shaped);
    assert!(
        missing.is_empty(),
        "tofu for {missing:?} — ⌘ must fall back through the default tail"
    );
    let adv = advances(&shaped);
    assert!(
        advances_eq(adv[0], adv[1]),
        "advances {adv:?} — Latin must stay on the mono face"
    );
}

/// An unknown family name is skipped and the stack falls through to the
/// engine default family (sans) — text renders, NOT tofu, with sans metrics.
#[test]
fn unknown_family_falls_through_to_default() {
    let mut mgr = wasm_font_stack();
    let shaped = shape(&mut mgr, Some("Nonexistent Sans"), None, "AI");
    assert!(
        tofu(&shaped).is_empty(),
        "tofu for {tofu:?} — unknown name must fall through to the default family",
        tofu = tofu(&shaped)
    );
    let adv = advances(&shaped);
    assert!(
        !advances_eq(adv[0], adv[1]),
        "advances {adv:?} — fall-through must land on the sans default, not mono"
    );
}

/// The explicit fallback list is consulted in order AFTER the primary: with
/// family "Nope Sans" (unknown) and fallback ["Roboto Mono"], text renders on
/// the mono face.
#[test]
fn fallback_list_is_consulted_in_order() {
    let mut mgr = wasm_font_stack();
    let fallback = ["Roboto Mono".to_string()];
    let shaped = shape(&mut mgr, Some("Nope Sans"), Some(&fallback), "AIl1.");
    assert!(tofu(&shaped).is_empty(), "tofu for the fallback-list shape");
    let adv = advances(&shaped);
    let uniform = adv.iter().all(|a| advances_eq(*a, adv[0]));
    assert!(
        uniform,
        "advances {adv:?} — the fallback list must supply the mono face"
    );
}

/// `fontFamily: None` keeps today's behavior: the default family (sans).
#[test]
fn no_family_still_defaults_to_sans() {
    let mut mgr = wasm_font_stack();
    let shaped = shape(&mut mgr, None, None, "AI");
    assert!(tofu(&shaped).is_empty(), "tofu for the default shape");
    let adv = advances(&shaped);
    assert!(
        !advances_eq(adv[0], adv[1]),
        "advances {adv:?} — the default family is sans"
    );
}
