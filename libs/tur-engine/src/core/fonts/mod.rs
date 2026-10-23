use std::sync::Arc;

pub use parley::FontContext;

/// Built-in platform-symbol fallback slice ("Tur Symbols", a renamed subset
/// of DejaVu Sans — see `LICENSE-TurSymbols.txt` next to this file): ⌘ U+2318,
/// ←↑→↓ U+2190-2193, ⇥ U+21E5, ⇧ U+21E7, ⌥ U+2325, ⌫ U+232B, ⏎ U+23CE.
///
/// The bundled Roboto faces (wasm) map none of these — the playground status
/// bar's "⌘S to run" rendered a tofu box — and stripped-down native systems
/// can lack them too. The slice rides in the engine binary and is installed
/// by [`install_symbol_fallback`] as the LAST face on the generic families,
/// so it is consulted only when the primary faces lack coverage.
const SYMBOL_FALLBACK_FONT: &[u8] = include_bytes!("TurSymbols-Regular.ttf");

/// Registers [`SYMBOL_FALLBACK_FONT`] and appends its family to the generic
/// families (sans/serif/mono) after the loader's own faces.
///
/// Must run AFTER `FontLoader::load_preset_fonts` — the wasm loader maps its
/// generics with `set_generic_families` (a REPLACE), which would drop an
/// earlier registration. We append to the generic families rather than the
/// fontique fallback map because `set/append_fallbacks` on a key BYPASSES the
/// platform system fallback for that script (the embedder entry wins outright),
/// while a generic-family append merely adds a last-resort face to the style
/// stack — the per-cluster query still consults system fallbacks afterwards.
fn install_symbol_fallback(fcx: &mut FontContext) {
    use parley::fontique::GenericFamily;

    let families = fcx
        .collection
        .register_fonts(SYMBOL_FALLBACK_FONT.to_vec().into(), None);
    let ids: Vec<_> = families.into_iter().map(|(id, _)| id).collect();
    for generic in [
        GenericFamily::SansSerif,
        GenericFamily::Serif,
        GenericFamily::Monospace,
    ] {
        fcx.collection
            .append_generic_families(generic, ids.iter().copied());
    }
}

/// The fresh-font-stack contract: preset fonts from the platform loader,
/// then the built-in symbol fallback (order matters — see
/// [`install_symbol_fallback`]). The ONE sequence a fresh [`FontContext`]
/// gets its fonts from; shared by `TurRuntime::build`, [`FontManager::new`]
/// and the coverage tests.
pub fn load_font_stack(fcx: &mut FontContext, loader: &dyn FontLoader) {
    loader.load_preset_fonts(fcx);
    install_symbol_fallback(fcx);
}

/// The font resolver: build the ordered family stack for a text style —
/// Flutter's model, verbatim.
///
/// `family` is a single family NAME resolved against the registered
/// families (aliases included); `fallback` is the explicit ordered fallback
/// list consulted after it; the engine default family (`sans-serif` — the
/// platform's sans on native, Roboto on wasm) is ALWAYS appended as the
/// tail, so a stack is never empty (Flutter's default-font invariant) and
/// the installed symbol face stays reachable as the last resort.
///
/// Unknown names are simply absent from the collection and skipped —
/// resolution moves to the next entry (Flutter's try-next behavior). Which
/// entry serves any given codepoint is decided per grapheme cluster by
/// parley's shaper walking this stack (and then fontique's per-script /
/// system fallback) — the `(fontFamily, codePoint)` pairing.
///
/// No string parsing, no generic keywords: hosts control what the default
/// tail maps to through their loader's generic-family mapping
/// ([`FontLoader::load_preset_fonts`]).
pub fn resolve_font_stack<'a>(
    family: Option<&'a str>,
    fallback: Option<&'a [String]>,
) -> parley::FontFamily<'a> {
    use parley::fontique::GenericFamily;
    use parley::style::{FontFamily, FontFamilyName};

    let mut stack: Vec<FontFamilyName<'a>> = Vec::new();
    if let Some(name) = family {
        stack.push(FontFamilyName::Named(name.into()));
    }
    if let Some(list) = fallback {
        stack.extend(
            list.iter()
                .map(|name| FontFamilyName::Named(name.as_str().into())),
        );
    }
    stack.push(FontFamilyName::Generic(GenericFamily::SansSerif));
    FontFamily::List(stack.into())
}

/// Font loading + registration. Implementations must be `Send + Sync` so
/// the runtime can hold them behind `Arc<dyn FontLoader + Send + Sync>`
/// and share across worker threads (Phase 8 threaded mode).
pub trait FontLoader: Send + Sync {
    fn load_preset_fonts(&self, fcx: &mut FontContext);
}

/// A declared face — pure typographic metadata, NO bytes, NO IO. The
/// declaration makes the family known to the lazy rail; the bytes arrive
/// later through [`FontRail::finish_load`] (the host's `FontDataProvider`).
///
/// The `overridden_*` fields are OVERRIDES, not identity inputs: `None`
/// means "the font file's own tables win" (variable fonts first-class);
/// `Some` names a specific static face point (a selector for the provider's
/// catalog and a placement for weight matching). They land on the
/// registration as fontique's `FontInfoOverride`.
#[derive(Debug, Clone, PartialEq)]
pub struct FontFaceMeta {
    pub family: String,
    pub overridden_weight: Option<u16>,
    pub overridden_style: Option<parley::FontStyle>,
    pub overridden_width: Option<parley::FontWidth>,
}

/// The plain face declaration — the family's default (all overrides `None`).
pub fn declared_face(family: impl Into<String>) -> FontFaceMeta {
    FontFaceMeta {
        family: family.into(),
        overridden_weight: None,
        overridden_style: None,
        overridden_width: None,
    }
}

/// A declared face's identity — the registry/dedup key. The enums are
/// keyed by their raw discriminants (fontique's `FontStyle`/`FontWidth`
/// don't impl `Eq`/`Hash`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct FaceKey {
    family: String,
    weight: Option<u16>,
    style: Option<u64>,
    width: Option<u64>,
}

fn face_key(meta: &FontFaceMeta) -> FaceKey {
    FaceKey {
        family: meta.family.clone(),
        weight: meta.overridden_weight,
        style: meta.overridden_style.map(style_key),
        width: meta.overridden_width.map(|w| w.ratio().to_bits() as u64),
    }
}

/// `parley::FontStyle`'s discriminant (`Normal`/`Italic` — copyable, hashable).
fn style_key(style: parley::FontStyle) -> u64 {
    match style {
        parley::FontStyle::Normal => 0,
        parley::FontStyle::Italic => 1,
        // Oblique carries an optional angle — key by the variant plus the
        // quantized angle bits (nobody spells oblique faces in a declare
        // today; the arm exists for exhaustiveness).
        parley::FontStyle::Oblique(angle) => {
            2u64.wrapping_add(angle.map(|a| (a as i64) as u64).unwrap_or(u64::MAX))
        }
    }
}

/// A declared face's lifecycle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FaceState {
    /// Declared, bytes not yet fetched.
    Declared,
    /// A load is in flight (claimed by [`FontRail::begin_load`]).
    Loading,
    /// Bytes registered — the face shapes.
    Loaded,
}

/// The shared font rail: ONE per runtime, `Arc`'d into every instance's
/// [`FontManager`] and the font rows' handles. Holds the declared-face
/// registry, the registration epoch (the layout-memo invalidation signal),
/// and its OWN clone of the runtime's shared `FontContext` — registering
/// through it lands in the shared collection store (fontique `shared:
/// true`), so every instance's next query pulls the new faces.
pub struct FontRail {
    epoch_cell: AtomicU64,
    inner: Mutex<FontRailInner>,
}

struct FontRailInner {
    fcx: FontContext,
    faces: HashMap<FaceKey, (FontFaceMeta, FaceState)>,
}

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

impl FontRail {
    /// Mint the rail over a clone of the runtime's (shared-collection)
    /// font context. `load_font_stack` must have run on it.
    pub fn new(fcx: FontContext) -> Self {
        Self {
            epoch_cell: AtomicU64::new(0),
            inner: Mutex::new(FontRailInner {
                fcx,
                faces: HashMap::new(),
            }),
        }
    }

    /// The registration epoch — advances on every successful
    /// [`Self::finish_load`]. Text layout memos fold it into their keys, so
    /// a landed font re-shapes every run that could now resolve differently.
    pub fn epoch(&self) -> u64 {
        self.epoch_cell.load(Ordering::Acquire)
    }

    /// Declare a face (idempotent by face identity — a repeat declare of
    /// the same identity is a no-op, even after load).
    pub fn declare(&self, meta: FontFaceMeta) {
        let mut inner = self.inner.lock().unwrap();
        inner.faces.entry(face_key(&meta)).or_insert((meta, FaceState::Declared));
    }

    /// Every declared face of `family` (any state).
    pub fn declared_faces(&self, family: &str) -> Vec<FontFaceMeta> {
        let inner = self.inner.lock().unwrap();
        inner
            .faces
            .values()
            .filter(|(meta, _)| meta.family == family)
            .map(|(meta, _)| meta.clone())
            .collect()
    }

    /// Claim `family`'s loadable faces (Declared → Loading). Returns the
    /// metas to fetch; empty while a load is already in flight or nothing
    /// is loadable (all loaded). The claim releases on [`Self::finish_load`].
    pub fn begin_load(&self, family: &str) -> Vec<FontFaceMeta> {
        let mut inner = self.inner.lock().unwrap();
        let mut claimed = Vec::new();
        for (key, (meta, state)) in inner.faces.iter_mut() {
            if key.family == family && *state == FaceState::Declared {
                *state = FaceState::Loading;
                claimed.push(meta.clone());
            }
        }
        claimed
    }

    /// Materialize a claimed face: register `data` under the DECLARED name
    /// (fontique's `FontInfoOverride` rename, honoring the `overridden_*`
    /// attrs) into the shared collection store, mark the face Loaded, and
    /// bump the epoch. `false` if the bytes carried no registerable face.
    pub fn finish_load(&self, meta: &FontFaceMeta, data: &[u8]) -> bool {
        use parley::fontique::FontInfoOverride;
        let override_ = FontInfoOverride {
            family_name: Some(&meta.family),
            weight: meta.overridden_weight.map(|w| parley::FontWeight::new(w as f32)),
            style: meta.overridden_style,
            width: meta.overridden_width,
            axes: None,
        };
        let registered = {
            let mut inner = self.inner.lock().unwrap();
            let families = inner
                .fcx
                .collection
                .register_fonts(data.to_vec().into(), Some(override_));
            let ok = !families.is_empty();
            if ok && let Some((_, state)) = inner.faces.get_mut(&face_key(meta)) {
                *state = FaceState::Loaded;
            }
            ok
        };
        if registered {
            self.epoch_cell.fetch_add(1, Ordering::AcqRel);
        }
        registered
    }
}

/// A fresh runtime font context: the shared-collection shape (`shared:
/// true` on both the collection store and the source cache) so runtime
/// registrations propagate to every instance clone, with system-font
/// discovery on (native/android; wasm's fontique has no backend so it is
/// naturally empty).
pub fn fresh_font_context() -> FontContext {
    FontContext {
        collection: parley::fontique::Collection::new(parley::fontique::CollectionOptions {
            shared: true,
            system_fonts: true,
        }),
        source_cache: parley::fontique::SourceCache::new(parley::fontique::SourceCacheOptions {
            shared: true,
        }),
    }
}

/// Per-instance font state. Wraps parley's [`FontContext`] (the font
/// database/layout scratch), the shared [`FontLoader`] (presets), and the
/// runtime's shared [`FontRail`] (declarations + epoch).
///
/// The expensive part — building the `FontContext` (system-font discovery +
/// preset-font loading) — happens **once** on the [`TurRuntime`](crate::TurRuntime)
/// and is cloned per instance: the collection store and the scanned system
/// data are `Arc`-backed, so a clone shares them; each instance keeps its
/// own fallback cache. Runtime registrations land through the rail's
/// shared store and are visible to every clone.
pub struct FontManager {
    inner: FontContext,
    rail: Arc<FontRail>,
}

impl FontManager {
    /// Wrap a (typically cloned) shared `FontContext`, the shared loader,
    /// and the runtime's rail. The caller is expected to have already loaded
    /// preset/system fonts into `fcx` once (on the runtime) — this does not
    /// re-load them.
    pub fn from_context(fcx: FontContext, rail: Arc<FontRail>) -> Self {
        Self { inner: fcx, rail }
    }

    /// Build a fresh `FontContext` (discovering system fonts) and load the
    /// loader's preset fonts into it. Used by standalone callers that don't
    /// share a runtime's pre-built context.
    pub fn new(loader: Arc<dyn FontLoader>) -> Self {
        let mut fcx = fresh_font_context();
        load_font_stack(&mut fcx, loader.as_ref());
        let rail = Arc::new(FontRail::new(fcx.clone()));
        Self::from_context(fcx, rail)
    }

    pub fn font_context(&mut self) -> &mut FontContext {
        &mut self.inner
    }

    /// The registration epoch (the rail's — the layout-memo invalidation
    /// signal; see [`FontRail::epoch`]).
    pub fn font_epoch(&self) -> u64 {
        self.rail.epoch()
    }

    /// The rail handle (the rows' declare/load path shares this Arc).
    pub fn rail(&self) -> Arc<FontRail> {
        self.rail.clone()
    }
}
