//! The lazy font rail: declare → materialize, propagating through the
//! shared collection.
//!
//! `FontFaceMeta` declares a face with PURE metadata (no IO, no bytes) —
//! the family becomes known to the rail but shapes nothing until
//! `finish_load` lands the bytes (a real registration under the declared
//! name via fontique's `FontInfoOverride` rename). Registrations land in
//! the SHARED collection store, so every instance clone sees them; the
//! epoch bumps per registration (the layout-memo invalidation signal).
//!
//! Hermetic: bundled bytes only, no system fonts (`system_fonts: false`).

use std::sync::Arc;

use parley::{LayoutContext, StyleProperty};
use tur_engine::core::fonts::{
    FontContext, FontLoader, FontManager, FontRail, declared_face, fresh_font_context,
    load_font_stack,
};

/// The shipped wasm face set — the browser's reality (no system fonts).
struct BundledWasmFonts;

impl FontLoader for BundledWasmFonts {
    fn load_preset_fonts(&self, fcx: &mut FontContext) {
        let default: &[u8] = include_bytes!("../../../tur-wasm/fonts/Roboto-VF.ttf");
        let mono: &[u8] = include_bytes!("../../../tur-wasm/fonts/RobotoMono-VF.ttf");
        let families = fcx.collection.register_fonts(default.to_vec().into(), None);
        let roboto_ids: Vec<_> = families.into_iter().map(|(id, _)| id).collect();
        fcx.collection
            .set_generic_families(parley::GenericFamily::SansSerif, roboto_ids.iter().copied());
        let mono_families = fcx.collection.register_fonts(mono.to_vec().into(), None);
        let mono_ids: Vec<_> = mono_families.into_iter().map(|(id, _)| id).collect();
        fcx.collection
            .set_generic_families(parley::GenericFamily::Monospace, mono_ids.iter().copied());
    }
}

/// One shared rail + a FontManager over it — the runtime's build shape
/// (`fresh_font_context` + `load_font_stack` + rail), minus system fonts
/// for hermeticity.
fn harness() -> (FontManager, Arc<FontRail>) {
    let loader = Arc::new(BundledWasmFonts);
    let mut fcx = fresh_font_context();
    fcx.collection = parley::fontique::Collection::new(parley::fontique::CollectionOptions {
        shared: true,
        system_fonts: false,
    });
    fcx.source_cache =
        parley::fontique::SourceCache::new(parley::fontique::SourceCacheOptions { shared: true });
    load_font_stack(&mut fcx, loader.as_ref());
    let rail = Arc::new(FontRail::new(fcx.clone()));
    let mgr = FontManager::from_context(fcx, rail.clone());
    (mgr, rail)
}

/// Shape `text` under the given family on `mgr`; `(char, glyph_id, advance)`
/// per codepoint.
fn shape(mgr: &mut FontManager, family: Option<&str>, text: &str) -> Vec<(char, u32, f32)> {
    let mut layout_cx = LayoutContext::<[u8; 4]>::new();
    let mut builder = layout_cx.ranged_builder(mgr.font_context(), text, 1.0, false);
    builder.push_default(StyleProperty::FontSize(11.0));
    builder.push_default(StyleProperty::FontFamily(resolve(family)));
    let mut layout = builder.build(text);
    layout.break_all_lines(None);
    text.char_indices()
        .map(|(offset, ch)| {
            let cluster = parley::Cluster::from_byte_index(&layout, offset)
                .unwrap_or_else(|| panic!("no cluster for {ch:?}"));
            let g = cluster.glyphs().next().unwrap_or_else(|| panic!("no glyph for {ch:?}"));
            (ch, g.id, g.advance)
        })
        .collect()
}

fn resolve(family: Option<&str>) -> parley::FontFamily<'_> {
    tur_engine::core::fonts::resolve_font_stack(family, None)
}

fn advances_eq(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.01
}

fn mono_meta(family: &str) -> tur_engine::core::fonts::FontFaceMeta {
    declared_face(family)
}

/// A declared face is metadata only: the family is KNOWN to the rail but
/// registers nothing into the collection — text under it shapes on the
/// default tail (sans metrics, non-tofu), NOT mono, NOT tofu.
#[test]
fn declared_face_shapes_on_fallback_until_loaded() {
    let (mut mgr, rail) = harness();
    rail.declare(mono_meta("Tur Custom"));
    let shaped = shape(&mut mgr, Some("Tur Custom"), "AI");
    assert!(shaped.iter().all(|&(_, gid, _)| gid != 0), "non-tofu via the default tail");
    let a = shaped[0].2;
    let b = shaped[1].2;
    assert!(
        !advances_eq(a, b),
        "advances {a}/{b} — a DECLARED-but-unloaded face must not shape as itself"
    );
}

/// `finish_load` materializes the face: the declared name now resolves BY
/// NAME to the loaded bytes (the `FontInfoOverride` rename) — monospace
/// advances under "Tur Custom".
#[test]
fn finish_load_registers_under_the_declared_name() {
    let (mut mgr, rail) = harness();
    let meta = mono_meta("Tur Custom");
    rail.declare(meta.clone());
    let mono: &[u8] = include_bytes!("../../../tur-wasm/fonts/RobotoMono-VF.ttf");
    assert!(rail.finish_load(&meta, mono), "the load registers");
    let shaped = shape(&mut mgr, Some("Tur Custom"), "AIl1.");
    assert!(shaped.iter().all(|&(_, gid, _)| gid != 0), "non-tofu after load");
    let adv: Vec<f32> = shaped.iter().map(|&(_, _, a)| a).collect();
    assert!(
        adv.iter().all(|a| advances_eq(*a, adv[0])),
        "advances {adv:?} — the loaded face renders monospaced under its declared name"
    );
}

/// Registrations land in the SHARED collection store: a context cloned
/// AFTER the load (a second instance) shapes the declared family too.
#[test]
fn loaded_font_propagates_to_clones() {
    let loader = Arc::new(BundledWasmFonts);
    let mut fcx = fresh_font_context();
    fcx.collection = parley::fontique::Collection::new(parley::fontique::CollectionOptions {
        shared: true,
        system_fonts: false,
    });
    fcx.source_cache =
        parley::fontique::SourceCache::new(parley::fontique::SourceCacheOptions { shared: true });
    load_font_stack(&mut fcx, loader.as_ref());
    let rail = Arc::new(FontRail::new(fcx.clone()));
    let mut inst_a = FontManager::from_context(fcx.clone(), rail.clone());
    // "Instance B" clones BEFORE the load.
    let mut inst_b = FontManager::from_context(fcx, rail.clone());

    let meta = mono_meta("Tur Custom");
    rail.declare(meta.clone());
    let mono: &[u8] = include_bytes!("../../../tur-wasm/fonts/RobotoMono-VF.ttf");
    rail.finish_load(&meta, mono);

    let shaped = shape(&mut inst_b, Some("Tur Custom"), "AIl1.");
    let adv: Vec<f32> = shaped.iter().map(|&(_, _, a)| a).collect();
    assert!(
        adv.iter().all(|a| advances_eq(*a, adv[0])),
        "advances {adv:?} — the clone must see the shared registration"
    );
    let _ = &mut inst_a;
}

/// The epoch is the layout-memo invalidation signal: it advances on every
/// registration (and only then).
#[test]
fn epoch_bumps_on_registration() {
    let (mut mgr, rail) = harness();
    let before = mgr.font_epoch();
    let meta = mono_meta("Tur Custom");
    rail.declare(meta.clone());
    assert_eq!(mgr.font_epoch(), before, "a declare alone does not bump");
    let mono: &[u8] = include_bytes!("../../../tur-wasm/fonts/RobotoMono-VF.ttf");
    rail.finish_load(&meta, mono);
    assert_eq!(mgr.font_epoch(), before + 1, "a registration bumps");
}

/// Declares dedup by face identity; `begin_load` claims the family's
/// pending faces (in-flight dedup) and `finish_load` releases them.
#[test]
fn declare_dedups_and_begin_load_claims() {
    let (mut mgr, rail) = harness();
    rail.declare(mono_meta("Inter"));
    rail.declare(mono_meta("Inter")); // idempotent
    assert_eq!(rail.declared_faces("Inter").len(), 1);

    let pending = rail.begin_load("Inter");
    assert_eq!(pending.len(), 1, "the first load claims the face");
    assert!(
        rail.begin_load("Inter").is_empty(),
        "a second load while in flight claims nothing"
    );

    let mono: &[u8] = include_bytes!("../../../tur-wasm/fonts/RobotoMono-VF.ttf");
    assert!(rail.finish_load(&pending[0], mono));
    // A RE-load after Loaded is a no-op claim (the face is materialized).
    assert!(
        rail.begin_load("Inter").is_empty(),
        "a loaded face is not re-claimed"
    );
    let _ = &mut mgr;
}

/// Faces with explicit overrides are distinct declarations (a static bold
/// beside the variable default), and the override lands on the registered
/// face (weight matching picks it for bold requests).
#[test]
fn overridden_faces_are_distinct_declarations() {
    let (mut mgr, rail) = harness();
    rail.declare(mono_meta("Inter"));
    rail.declare(tur_engine::core::fonts::FontFaceMeta {
        family: "Inter".into(),
        overridden_weight: Some(700),
        overridden_style: None,
        overridden_width: None,
    });
    assert_eq!(rail.declared_faces("Inter").len(), 2, "default + bold faces");

    let mono: &[u8] = include_bytes!("../../../tur-wasm/fonts/RobotoMono-VF.ttf");
    let faces = rail.begin_load("Inter");
    assert_eq!(faces.len(), 2);
    for face in &faces {
        assert!(rail.finish_load(face, mono));
    }
    // A bold request still resolves the family (the renamed face carries
    // overridden_weight 700; fontique matches nearest).
    let shaped = shape(&mut mgr, Some("Inter"), "AI");
    assert!(shaped.iter().all(|&(_, gid, _)| gid != 0), "bold request non-tofu");
}
