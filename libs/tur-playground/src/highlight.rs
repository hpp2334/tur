//! The playground's syntax highlighting — semantic classes for rut source
//! as a flat colored span run for the editor controller (the boa
//! `buildHighlightSpans` rail).
//!
//! The classifier is the upstream one again, over its new public lane:
//! the **rut-lsp actor** (`Lsp::analyze`'s semantic-token stream — the
//! same `crate::semantic` classifier the old `rut-semantic` crate
//! exposed, sealed by the privacy wall but served through the ABI).
//! The actor is driven INLINE: the loop future rides a `futures`
//! LocalPool (booted through `Lsp::boot`'s spawn seam — `spawn_native`
//! is native-gated upstream, but a tur-owned pool compiles on wasm32
//! too), so `pool.run_until(lsp.analyze(..))` answers synchronously on
//! the worker lane with no extra thread, no runtime, no parking — one
//! LSP session per thread, booted lazily.
//!
//! The pipeline: `lex_canonical` (normalize → lex, one call) gives the
//! canonical text + the comment side table; the normalized text rides
//! `analyze` (LSP positions index exactly the text we send, so the join
//! contract keeps holding); the answer's delta-encoded tokens
//! (UTF-16 line/char/len — LSP's currency) decode back to byte spans
//! against the same text; the legend names map onto the palette table
//! below; the tiling fill covers what the legend leaves out. Structural
//! punctuation (commas, semicolons, braces, dots) is unclassified and
//! has no literal class, so the fill colors the gap bytes: whitespace
//! stretches plain, punctuation operators, and tur's contextual item
//! keyword `entry` (`entry fn` — outside upstream's keyword table)
//! keyword teal. The runs join back to exactly the normalized source
//! (`set_spans` replaces the document; CRLF normalization stays free).
//!
//! Zero-width runs are skipped (an empty-content span creates an empty
//! parley style range and panics text layout — the trap the boa code
//! comments). Colors are the boa tokens' `code.*` palette (light theme, AA
//! on `code.bg`), restated as packed `0xRRGGBBAA` (the kit color law).
//!
//! Metric note (phase-4 P0): a "wide inter-word gaps in comments" report
//! measured out clean — the per-comment runs shape with the same monospace
//! advances as the code runs (parley `Glyph.x` is a per-glyph offset, and
//! `extract_layout_data` accumulates correctly). The impression comes from
//! the light comment ink (the palette, phase-2 territory). Pinned by
//! `editor_comment_spans_keep_uniform_monospace_advances` in the
//! playground gate.

use rut_lexer::span::Span;
use rut_lexer::lexer::lex_canonical;
use rut_lsp::Lsp;
use futures::task::LocalSpawnExt;

use tur_engine::builtin_plugins::text::controller::SpanData;
use tur_engine::core::rut_runtime::color_of;

// The code-highlight palette — the boa `tokens.ts` `code.*` values.
const CODE_FG: u64 = 0x1F2530FF; // code.fg — ink.800 (plain text)
const CODE_KEYWORD: u64 = 0x006E58FF; // code.keyword — teal.700
const CODE_STRING: u64 = 0x3F7D3FFF; // code.string
const CODE_NUMBER: u64 = 0xB35900FF; // code.number
const CODE_COMMENT: u64 = 0x8A94A3FF; // code.comment — ink.500
const CODE_OPERATOR: u64 = 0x5E6878FF; // code.operator — ink.600
const CODE_LITERAL: u64 = 0x92400EFF; // code.literal (true / false / nil)
const CODE_FUNCTION: u64 = 0x1D4ED8FF; // code.function — fn + method names
const CODE_TYPE: u64 = 0x6D28D9FF; // code.type — type/class/enum names

/// The `pg_highlight` payload: the classified source as a flat colored
/// span run (text + color only), sealed for the rut realm as an opaque.
pub struct PgSpans(pub Vec<SpanData>);

/// The legend class each semantic token carries — the upstream legend's
/// names restated as one enum (Variable/Parameter/Property were always
/// plain ink; one `Plain` class carries them).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Class {
    Keyword,
    Number,
    String,
    Comment,
    Operator,
    Literal,
    Function,
    Type,
    Plain,
}

impl Class {
    fn color(self) -> u64 {
        match self {
            Class::Keyword => CODE_KEYWORD,
            Class::Number => CODE_NUMBER,
            Class::String => CODE_STRING,
            Class::Comment => CODE_COMMENT,
            Class::Operator => CODE_OPERATOR,
            Class::Literal => CODE_LITERAL,
            Class::Function => CODE_FUNCTION,
            Class::Type => CODE_TYPE,
            Class::Plain => CODE_FG,
        }
    }

    /// One legend name → its class; `None` skips the token (unknown
    /// future legend rows fall to the gap fill, never a wrong color).
    fn of_legend(name: &str) -> Option<Class> {
        Some(match name {
            "keyword" => Class::Keyword,
            "number" => Class::Number,
            "string" => Class::String,
            "function" | "method" => Class::Function,
            "type" | "enum" | "class" | "interface" | "namespace" => Class::Type,
            "enumMember" => Class::Literal,
            "variable" | "parameter" | "property" => Class::Plain,
            "operator" => Class::Operator,
            _ => return None,
        })
    }
}

/// One LSP editing session, driven inline: the actor's loop future
/// rides the pool, so every bridge answers via `pool.run_until(..)` —
/// synchronous from the caller's seat, single-threaded, wasm-safe.
/// One per thread (lazily booted; the session state is the LSP's own,
/// prelude pre-indexed at boot).
struct Session {
    lsp: Lsp,
    pool: futures::executor::LocalPool,
    /// legend names in token-type index order (decoded once).
    legend: Vec<String>,
}

thread_local! {
    static SESSION: std::cell::RefCell<Session> = std::cell::RefCell::new(boot_session());
}

/// Boot the session: the loop onto a tur-owned pool through
/// `Lsp::boot`'s spawn seam, the legend through the same inline drive.
fn boot_session() -> Session {
    let mut pool = futures::executor::LocalPool::new();
    let spawner = pool.spawner().clone();
    let lsp = Lsp::boot(move |fut| {
        spawner
            .spawn_local(fut)
            .expect("the highlight loop parks on the fresh pool");
    });
    let legend_json = pool.run_until(lsp.legend());
    let legend: Vec<String> = serde_json::from_str(&legend_json).unwrap_or_default();
    Session { lsp, pool, legend }
}

/// The editor doc's URI — a synthetic one; the session is the
/// highlight's own (full-sync every call, `analyze` stores first).
const EDITOR_URI: &str = "file:///tur-editor.rut";

/// Classify `src` into the colored span run the editor controller renders
/// (text + color only; every other style inherits the element's defaults).
/// The join of the answer's texts is the CRLF-normalized source — spans
/// index the normalized text (the classifier analyzes exactly the text we
/// send), and applying the run normalizes the editor content for free.
pub fn highlight_spans(src: &str) -> Vec<SpanData> {
    let (norm, _toks, _diags, comments, _lines) = lex_canonical(src.to_owned(), true);
    let classes = classify(&norm);
    let mut classes = classes;
    classes.extend(comments.iter().map(|c| (c.span, Class::Comment)));
    classes.sort_by_key(|(span, _)| span.lo);
    let mut b = Builder {
        src: &norm,
        spans: Vec::new(),
        pos: 0,
    };
    let mut pos = 0usize;
    for (span, class) in classes {
        let lo = span.lo as usize;
        let hi = span.hi as usize;
        b.gap(pos, lo);
        // Keywords whose text is a literal keep the literal brown
        // (`true` / `false` / `nil` classify as keywords — the legend
        // has no literal class).
        let color = if class == Class::Keyword && matches!(&norm[lo..hi], "true" | "false" | "nil")
        {
            CODE_LITERAL
        } else {
            class.color()
        };
        b.emit(lo, hi, color);
        pos = hi.max(pos);
    }
    b.gap(pos, norm.len());
    b.spans
}

/// The classifier lane: `analyze` over the normalized text, the answer's
/// delta-encoded tokens decoded back to byte spans. A refused envelope
/// (the loop gone, an internal error) answers empty — the fill paints
/// the whole source (never a wrong color, the degraded-but-safe lane).
fn classify(norm: &str) -> Vec<(Span, Class)> {
    SESSION.with(|cell| {
        let mut s = cell.borrow_mut();
        let Session { lsp, pool, .. } = &mut *s;
        let json = pool.run_until(lsp.analyze(EDITOR_URI.to_string(), norm.to_string()));
        decode_tokens(&json, &s.legend, norm).unwrap_or_default()
    })
}

/// Decode `analyze`'s envelope: the flat 5-tuple delta stream
/// (Δline, Δchar, len, typeIdx, modifiers — `data` is ONE flat u32
/// array, the LSP wire shape) over UTF-16 line/char positions →
/// absolute byte spans over `norm`.
fn decode_tokens(json: &str, legend: &[String], norm: &str) -> Option<Vec<(Span, Class)>> {
    let value: serde_json::Value = serde_json::from_str(json).ok()?;
    let flat = value.get("tokens")?.get("data")?.as_array()?;
    let mut n = [0u32; 5];
    let mut filled = 0usize;
    let mut line_starts: Vec<u32> = vec![0];
    for (i, b) in norm.bytes().enumerate() {
        if b == b'\n' {
            line_starts.push(i as u32 + 1);
        }
    }
    let mut out = Vec::with_capacity(flat.len() / 5);
    let (mut line, mut ch) = (0u32, 0u32);
    for v in flat {
        n[filled] = v.as_u64()? as u32;
        filled += 1;
        if filled < 5 {
            continue;
        }
        filled = 0;
        let (dl, dc, len, ty) = (n[0], n[1], n[2], n[3]);
        if dl > 0 {
            line += dl;
            ch = dc;
        } else {
            ch += dc;
        }
        let class = match legend.get(ty as usize).and_then(|n| Class::of_legend(n)) {
            Some(c) => c,
            // an unknown legend row skips ITS token — never the whole
            // stream (a future class falls to the gap fill)
            None => continue,
        };
        let line_start = *line_starts.get(line as usize)? as usize;
        let line_text = norm[line_start..].split('\n').next().unwrap_or("");
        let lo = utf16_to_byte(line_text, ch)?;
        let hi = utf16_to_byte(line_text, ch + len)?;
        if hi > lo {
            out.push((
                Span {
                    lo: (line_start + lo) as u32,
                    hi: (line_start + hi) as u32,
                },
                class,
            ));
        }
    }
    (filled == 0).then_some(out)
}

/// UTF-16 column → byte offset within one line (LSP's position
/// currency; the source is the canonical text, so the mapping is exact).
fn utf16_to_byte(line: &str, col16: u32) -> Option<usize> {
    let mut units = 0u32;
    for (i, c) in line.char_indices() {
        if units >= col16 {
            return Some(i);
        }
        units += c.len_utf16() as u32;
    }
    (units == col16).then_some(line.len())
}

/// The run builder: appends non-overlapping colored runs in source order,
/// merging runs that are contiguous AND same-colored (keeps the span list
/// at one run per color stretch, not one per token). Zero-width and
/// backwards emits are dropped — the empty-parley-style-range trap.
struct Builder<'a> {
    src: &'a str,
    spans: Vec<SpanData>,
    pos: usize,
}

impl<'a> Builder<'a> {
    fn emit(&mut self, lo: usize, hi: usize, color: u64) {
        if lo >= hi {
            return; // zero-width — an empty span would panic parley layout
        }
        if lo > self.pos {
            self.push(self.pos, lo, CODE_FG); // never happens; safety net
        }
        self.push(lo, hi, color);
    }

    fn push(&mut self, lo: usize, hi: usize, color: u64) {
        let text = &self.src[lo..hi];
        if text.is_empty() {
            return;
        }
        if lo == self.pos
            && let Some(last) = self.spans.last_mut()
            && last.color() == Some(color_of(color))
        {
            last.text.push_str(text);
            self.pos = hi;
            return;
        }
        self.spans.push(SpanData::colored(text, color_of(color)));
        self.pos = hi;
    }

    /// The unclassified gap `[lo, hi)`: whitespace stays plain, structural
    /// punctuation colors as an operator (the legend leaves it out —
    /// the boa look survives), and the contextual item keyword `entry`
    /// keeps its keyword ink. Keywords whose text is a literal keep the
    /// literal brown (`true` / `false` / `nil` classify as keywords —
    /// the legend has no literal class).
    fn gap(&mut self, lo: usize, hi: usize) {
        let bytes = self.src.as_bytes();
        let mut i = lo;
        while i < hi {
            let ws = bytes[i].is_ascii_whitespace();
            let mut j = i + 1;
            while j < hi && bytes[j].is_ascii_whitespace() == ws {
                j += 1;
            }
            let text = &self.src[i..j];
            let color = if ws {
                CODE_FG
            } else if matches!(text, "true" | "false" | "nil") {
                CODE_LITERAL
            } else if text == "entry" {
                CODE_KEYWORD
            } else {
                CODE_OPERATOR
            };
            self.emit(i, j, color);
            i = j;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The hex triplet each span's color must carry (the palette probe).
    fn rgbs(spans: &[SpanData]) -> Vec<(String, [u8; 3])> {
        spans
            .iter()
            .map(|s| {
                let c = s.color().expect("every highlight span carries a color");
                (s.text.clone(), [c.r(), c.g(), c.b()])
            })
            .collect()
    }

    const FG: [u8; 3] = [0x1F, 0x25, 0x30]; // ink.800 — code.fg
    const KW: [u8; 3] = [0x00, 0x6E, 0x58]; // teal.700 — code.keyword
    const STR: [u8; 3] = [0x3F, 0x7D, 0x3F]; // code.string
    const NUM: [u8; 3] = [0xB3, 0x59, 0x00]; // code.number
    const COMMENT: [u8; 3] = [0x8A, 0x94, 0xA3]; // ink.500 — code.comment
    const OP: [u8; 3] = [0x5E, 0x68, 0x78]; // ink.600 — code.operator
    const LITERAL: [u8; 3] = [0x92, 0x40, 0x0E]; // code.literal
    const FUNCTION: [u8; 3] = [0x1D, 0x4E, 0xD8]; // code.function
    const TYPE: [u8; 3] = [0x6D, 0x28, 0xD9]; // code.type

    #[test]
    fn keyword_string_comment_classify() {
        let spans = highlight_spans("let s = \"hi\"; // c\n");
        // The runs join back to the source…
        let joined: String = spans.iter().map(|s| s.text.as_str()).collect();
        assert_eq!(joined, "let s = \"hi\"; // c\n");
        // …and every stretch carries its palette color: keyword, plain,
        // operator, plain, string, operator, plain, comment, plain.
        assert_eq!(
            rgbs(&spans),
            vec![
                ("let".into(), KW),
                (" s ".into(), FG),
                ("=".into(), OP),
                (" ".into(), FG),
                ("\"hi\"".into(), STR),
                (";".into(), OP),
                (" ".into(), FG),
                ("// c".into(), COMMENT),
                ("\n".into(), FG),
            ]
        );
    }

    #[test]
    fn numbers_bools_and_punctuation_classify() {
        let spans = highlight_spans("f(12.5, true)");
        assert_eq!(
            rgbs(&spans),
            vec![
                // A top-level call is part of no item — the callee rides
                // the operator gap together with the paren.
                ("f(".into(), OP),
                ("12.5".into(), NUM),
                (",".into(), OP),
                (" ".into(), FG),
                ("true".into(), LITERAL),
                (")".into(), OP),
            ]
        );
    }

    #[test]
    fn entry_and_keywords_color_nil_is_a_literal() {
        let spans = highlight_spans("entry fn start() -> str { return nil; }");
        let got = rgbs(&spans);
        // The run carrying `needle` (whole-word runs for the keywords, the
        // merged plain stretch for `str`).
        let at = |needle: &str| {
            got.iter()
                .find(|(t, _)| t.contains(needle))
                .unwrap_or_else(|| panic!("no span with `{needle}` in {got:?}"))
                .1
        };
        assert_eq!(at("entry"), KW);
        assert_eq!(at("fn"), KW);
        assert_eq!(at("return"), KW);
        assert_eq!(at("str"), TYPE, "primitive type names classify as types");
        assert_eq!(at("nil"), LITERAL);
        assert_eq!(at("->"), OP);
        // The runs join back to the source.
        let joined: String = spans.iter().map(|s| s.text.as_str()).collect();
        assert_eq!(joined, "entry fn start() -> str { return nil; }");
    }

    #[test]
    fn string_raw_string_and_fstring_holes() {
        let spans = highlight_spans("let x = f\"x {name} y\" + r\"raw\";\n");
        assert_eq!(
            rgbs(&spans),
            vec![
                ("let".into(), KW),
                (" x ".into(), FG),
                ("=".into(), OP),
                (" ".into(), FG),
                // The literal's prologue colors as a string (through the
                // `{`)…
                ("f\"x {".into(), STR),
                // …the hole's identifier classifies (a variable — plain)…
                ("name".into(), FG),
                // …the hole's closing brace rides the operator gap, and the
                // tail after it stays string.
                ("}".into(), OP),
                (" y\"".into(), STR),
                (" ".into(), FG),
                ("+".into(), OP),
                (" ".into(), FG),
                ("r\"raw\"".into(), STR),
                (";".into(), OP),
                ("\n".into(), FG),
            ]
        );
    }

    #[test]
    fn block_comment_and_crlf_normalize() {
        let spans = highlight_spans("let a = 1; /* mid */ let b = 2;\r\n// tail\r\n");
        let joined: String = spans.iter().map(|s| s.text.as_str()).collect();
        assert_eq!(joined, "let a = 1; /* mid */ let b = 2;\n// tail\n");
        let got = rgbs(&spans);
        let at = |text: &str| {
            got.iter()
                .find(|(t, _)| t == text)
                .unwrap_or_else(|| panic!("no span `{text}` in {got:?}"))
                .1
        };
        assert_eq!(at("/* mid */"), COMMENT);
        assert_eq!(at("// tail"), COMMENT);
        assert_eq!(at("let"), KW);
    }

    #[test]
    fn empty_and_unterminated_sources_stay_flat() {
        assert!(highlight_spans("").is_empty(), "no source, no spans");
        // Unterminated string: the lexer reports a diag but still yields a
        // token run; nothing is lost and the literal colors as a string.
        let spans = highlight_spans("let s = \"oops");
        let joined: String = spans.iter().map(|s| s.text.as_str()).collect();
        assert_eq!(joined, "let s = \"oops");
        let got = rgbs(&spans);
        assert!(
            got.contains(&("\"oops".to_string(), STR)),
            "the unterminated literal colors as a string: {got:?}"
        );
    }

    #[test]
    fn spans_never_overlap_or_skip_text() {
        let src = "use tur_host::{ mount };\nentry fn start() {\n    let x = f\"{x + 1}\"; // t\n}\n";
        let spans = highlight_spans(src);
        let mut pos = 0usize;
        for s in &spans {
            assert!(!s.text.is_empty(), "zero-width span (the parley trap)");
            assert!(s.color().is_some(), "every span carries a color");
            pos += s.text.len();
        }
        assert_eq!(
            pos,
            rut_lexer::lexer::canonical(src).len(),
            "runs tile the source"
        );
    }

    /// The color of the first run whose text contains `needle` — for names
    /// that merge with an adjacent plain gap (a `let` binding always does:
    /// `let full` colors as one plain stretch).
    fn at(got: &[(String, [u8; 3])], needle: &str) -> [u8; 3] {
        got.iter()
            .find(|(t, _)| t.contains(needle))
            .unwrap_or_else(|| panic!("no span with `{needle}` in {got:?}"))
            .1
    }

    #[test]
    fn fn_decl_call_and_binding_roles_color() {
        let spans =
            highlight_spans("fn greet(n: u64) -> str {\n    let full = hi(n);\n}\n");
        let got = rgbs(&spans);
        assert_eq!(at(&got, "greet"), FUNCTION, "the fn decl name");
        assert_eq!(at(&got, "hi"), FUNCTION, "the call callee");
        assert_eq!(at(&got, "full"), FG, "the let binding stays plain");
        assert_eq!(at(&got, "u64"), TYPE, "the primitive param type");
        assert_eq!(at(&got, "str"), TYPE, "the return type");
        // Exact runs: the param and the call arg sit between punctuation,
        // so they never merge — both plain.
        let exact = |text: &str| {
            got.iter()
                .find(|(t, _)| t == text)
                .unwrap_or_else(|| panic!("no exact span `{text}` in {got:?}"))
                .1
        };
        assert_eq!(exact("n"), FG, "the param + the call arg stay plain");
        assert_eq!(exact("fn"), KW);
    }

    #[test]
    fn method_call_names_color_function() {
        let spans = highlight_spans("fn go(s: str) {\n    s.draw();\n}\n");
        let got = rgbs(&spans);
        let exact = |text: &str| {
            got.iter()
                .find(|(t, _)| t == text)
                .unwrap_or_else(|| panic!("no exact span `{text}` in {got:?}"))
                .1
        };
        assert_eq!(exact("go"), FUNCTION, "the fn decl name");
        assert_eq!(exact("draw"), FUNCTION, "the method-call name");
        assert_eq!(exact("s"), FG, "the param stays plain");
        assert_eq!(exact("str"), TYPE, "the primitive param type");
    }

    #[test]
    fn capitalized_path_segs_follow_the_type_convention() {
        let spans = highlight_spans("fn go(c: Color) {\n    let r = Color::Red;\n}\n");
        let got = rgbs(&spans);
        // The `::` path law: capitalized path segs read as types, a
        // capitalized last seg as an enum member (`.` is value access only).
        assert!(
            got.iter().all(|(t, c)| t != "Color" || *c == TYPE),
            "every `Color` run colors as a type: {got:?}"
        );
        let exact = |text: &str| {
            got.iter()
                .find(|(t, _)| t == text)
                .unwrap_or_else(|| panic!("no exact span `{text}` in {got:?}"))
                .1
        };
        assert_eq!(exact("Red"), LITERAL, "the capitalized member");
        assert_eq!(exact("c"), FG, "the param stays plain");
        assert_eq!(exact("go"), FUNCTION);
    }
}
