//! The playground's syntax highlighting — semantic classes for rut source
//! as a flat colored span run for the editor controller (the boa
//! `buildHighlightSpans` rail, now over the upstream `rut-semantic`
//! classifier instead of token-text classes alone).
//!
//! The pipeline: `rut_semantic::classify_source` (normalize → lex →
//! best-effort parse → token + AST name classes → comment gap scan; broken
//! source degrades to token classes — never a wrong color) → the palette
//! table below → the tiling fill. The legend deliberately leaves
//! structural punctuation (commas, semicolons, braces, dots) unclassified
//! and has no literal class, so the fill colors the gap bytes: whitespace
//! stretches plain, punctuation operators, and tur's contextual item
//! keyword `entry` (`entry fn` — outside upstream's reserved table)
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

use rut_semantic::TokenType;

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

/// Classify `src` into the colored span run the editor controller renders
/// (text + color only; every other style inherits the element's defaults).
/// The join of the answer's texts is the CRLF-normalized source — spans
/// index the normalized text (`classify_source`'s contract, normalized
/// identically here), and applying the run normalizes the editor content
/// for free.
pub fn highlight_spans(src: &str) -> Vec<SpanData> {
    let norm = rut_lexer::lexer::normalize(src);
    let classes = rut_semantic::classify_source(src, rut_parser::Mode::Impl);
    let mut b = Builder {
        src: &norm,
        spans: Vec::new(),
        pos: 0,
    };
    let mut pos = 0usize;
    for (span, ty) in &classes {
        let lo = span.lo as usize;
        let hi = span.hi as usize;
        b.gap(pos, lo);
        b.emit(lo, hi, classify(*ty, &norm[lo..hi]));
        pos = hi.max(pos);
    }
    b.gap(pos, norm.len());
    b.spans
}

/// One legend class → its palette color. Keywords whose text is a literal
/// keep the literal brown (`true` / `false` / `nil` classify as keywords —
/// the legend has no literal class). `Variable` / `Parameter` /
/// `Property` stay plain ink (distinct colors deferred — one table row
/// each, later).
fn classify(ty: TokenType, text: &str) -> u64 {
    match ty {
        TokenType::Keyword if matches!(text, "true" | "false" | "nil") => CODE_LITERAL,
        TokenType::Keyword => CODE_KEYWORD,
        TokenType::Number => CODE_NUMBER,
        TokenType::String => CODE_STRING,
        TokenType::Comment => CODE_COMMENT,
        TokenType::Operator => CODE_OPERATOR,
        TokenType::EnumMember => CODE_LITERAL,
        TokenType::Function | TokenType::Method => CODE_FUNCTION,
        TokenType::Type | TokenType::Enum | TokenType::Class | TokenType::Interface => CODE_TYPE,
        TokenType::Variable | TokenType::Parameter | TokenType::Property => CODE_FG,
    }
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
    /// punctuation colors as an operator (the legend leaves it out — the
    /// boa look survives), and the contextual item keyword `entry` keeps
    /// its keyword ink.
    fn gap(&mut self, lo: usize, hi: usize) {
        let bytes = self.src.as_bytes();
        let mut i = lo;
        while i < hi {
            let ws = bytes[i].is_ascii_whitespace();
            let mut j = i + 1;
            while j < hi && bytes[j].is_ascii_whitespace() == ws {
                j += 1;
            }
            let color = if ws {
                CODE_FG
            } else if j - i == 5 && &self.src[i..j] == "entry" {
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
            rut_lexer::lexer::normalize(src).len(),
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
        let spans = highlight_spans("fn go(c: Color) {\n    let r = Color.Red;\n}\n");
        let got = rgbs(&spans);
        // The value position has no resolution yet — capitalized prefix
        // segs read as types, a capitalized last seg as an enum member.
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
