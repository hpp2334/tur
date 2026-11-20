//! The playground's syntax highlighting — colored span classes for rut
//! source as a flat colored span run for the editor controller (the boa
//! `buildHighlightSpans` rail).
//!
//! The pipeline is a TOKEN-WALK classifier tur-side: `lex_canonical`
//! (normalize → lex, one call) gives the token stream + the comment side
//! table, and the walker below classes every token — keywords by the
//! upstream reserved table, literals by token kind, and names by a small
//! positional rule set (fn-decl names, call/method callees, the `::` path
//! conventions, the primitive type table). It replaced the upstream
//! `rut-semantic` classifier, which the rut-lsp merge sealed behind
//! rut-lsp's privacy wall (`Lsp`/`Io` are its only public doors — the
//! async actor JSON envelopes are no lane for a sync in-process
//! highlighter); the pinned color behavior is unchanged.
//!
//! Un-classified bytes fall to the tiling fill: whitespace stretches
//! plain, structural punctuation operators, and tur's contextual item
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

use rut_ast::parser::{PRIMITIVE_TYS, RESERVED_KW};
use rut_lexer::lexer::lex_canonical;
use rut_lexer::span::Span;
use rut_lexer::token::{FPart, Tok, Token};

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

/// The legend class each token walks into — the old upstream classifier's
/// visible classes, restated tur-side (Variable/Parameter/Property were
/// always plain ink; one `Plain` class carries them).
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
}

/// Classify `src` into the colored span run the editor controller renders
/// (text + color only; every other style inherits the element's defaults).
/// The join of the answer's texts is the CRLF-normalized source — spans
/// index the normalized text (the lexer's canonical text contract), and
/// applying the run normalizes the editor content for free.
pub fn highlight_spans(src: &str) -> Vec<SpanData> {
    let (norm, toks, _diags, comments, _lines) = lex_canonical(src.to_owned(), true);
    let mut classes: Vec<(Span, Class)> = Vec::new();
    walk(&toks, 0, &mut classes);
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
        b.emit(lo, hi, class.color());
        pos = hi.max(pos);
    }
    b.gap(pos, norm.len());
    b.spans
}

/// The token walker: classes every token, tracking brace depth for the
/// callee rule (a call inside a body colors its callee `Function`; a
/// top-level bare call — part of no item, the old classifier's gap —
/// stays unclassified so the fill paints it operator, byte-faithful to
/// the pinned behavior).
fn walk(toks: &[Token], mut depth: u32, out: &mut Vec<(Span, Class)>) {
    for (i, t) in toks.iter().enumerate() {
        match &t.tok {
            Tok::Int(..) | Tok::Float(..) => out.push((t.span, Class::Number)),
            Tok::Str(_) | Tok::RawStr(_) => out.push((t.span, Class::String)),
            Tok::FStr(f) => fstring(f, t.span, depth, out),
            Tok::Bool(_) => out.push((t.span, Class::Literal)),
            Tok::Ident(name) => {
                if let Some(class) = ident_class(name, i, toks, depth) {
                    out.push((t.span, class));
                }
            }
            Tok::LBrace => {
                out.push((t.span, Class::Operator));
                depth += 1;
            }
            Tok::RBrace => {
                depth = depth.saturating_sub(1);
                out.push((t.span, Class::Operator));
            }
            Tok::Eof => {}
            _ => out.push((t.span, Class::Operator)),
        }
    }
}

/// One f-string literal: the prologue, the stretches between holes, and
/// the tail color as the string; each hole's token stream (spans real,
/// the closing `}` included) recurses through the walker — hole names
/// classify by the same rules, the hole braces read as operators.
fn fstring(f: &rut_lexer::token::FStrTok, span: Span, depth: u32, out: &mut Vec<(Span, Class)>) {
    let mut cursor = span.lo;
    for part in &f.parts {
        if let FPart::Hole(hole) = part
            && let Some(first) = hole.first()
        {
            if cursor < first.span.lo {
                out.push((Span { lo: cursor, hi: first.span.lo }, Class::String));
            }
            walk(hole, depth, out);
            cursor = hole.last().map_or(first.span.lo, |t| t.span.hi);
        }
    }
    if cursor < span.hi {
        out.push((Span { lo: cursor, hi: span.hi }, Class::String));
    }
}

/// One identifier's class by text + positional context, or `None` to
/// leave it unclassified (the fill paints it — the top-level callee gap).
#[allow(clippy::too_many_lines)]
fn ident_class(name: &str, i: usize, toks: &[Token], depth: u32) -> Option<Class> {
    // `nil` — the null literal rode the keyword table before the legend
    // grew no literal class; it keeps the literal brown.
    if name == "nil" {
        return Some(Class::Literal);
    }
    // tur's contextual item keyword (upstream's reserved table keeps its
    // distance — `entry fn` is a tur spelling).
    if name == "entry" {
        return Some(Class::Keyword);
    }
    if RESERVED_KW.contains(&name) {
        return Some(Class::Keyword);
    }
    if PRIMITIVE_TYS.contains(&name) {
        return Some(Class::Type);
    }
    let prev_is = |text: &str| i > 0 && matches!(&toks[i - 1].tok, Tok::Ident(k) if k == text);
    let next_is_lparen = matches!(toks.get(i + 1).map(|t| &t.tok), Some(Tok::LParen));
    // decl names: `fn f` colors function; a type decl head colors type.
    if prev_is("fn") {
        return Some(Class::Function);
    }
    if prev_is("class") || prev_is("enum") || prev_is("interface") {
        return Some(Class::Type);
    }
    // The `::` path law: capitalized path segs read as types, a
    // capitalized last seg as an enum member (`.` is value access only).
    if name
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_uppercase())
    {
        let after_path = matches!(&toks[i.saturating_sub(1)].tok, Tok::Colon2);
        return Some(if after_path { Class::Literal } else { Class::Type });
    }
    // Call + method-call callees (a method rides `.name(`). A TOP-LEVEL
    // bare call is part of no item — unclassified, so the fill paints
    // it operator (the old classifier's gap, byte-faithful).
    if next_is_lparen {
        if depth > 0 {
            return Some(Class::Function);
        }
        return None;
    }
    Some(Class::Plain)
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
    /// punctuation colors as an operator (the classifier leaves it out —
    /// the boa look survives), and the contextual item keyword `entry`
    /// keeps its keyword ink.
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
