//! The playground's syntax highlighting — tokenize rut source into a flat
//! colored span run for the editor controller (the boa
//! `buildHighlightSpans` rail, lexical only).
//!
//! The pipeline mirrors the boa reference (READ ONLY
//! `tur-boa/.../cases/compile.ts`): lex → per-token colors → span runs,
//! zero-width runs skipped (an empty-content span creates an empty parley
//! style range and panics text layout — the trap the boa code comments).
//! Colors are the boa tokens' `code.*` palette (light theme, AA on
//! `code.bg`), restated as packed `0xRRGGBBAA` (the kit color law).
//!
//! rut-lexer is a mode-stack tokenizer that SKIPS comments and whitespace —
//! they are the gaps between tokens — so the builder scans each gap for
//! `//` / `/* */` runs itself. `f"..."` interpolation holes are fully
//! lexed token streams with absolute spans inside the literal: the literal
//! colors as a string and the hole tokens overlay their own colors.

use rut_lexer::token::{FPart, Tok};

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

/// The `pg_highlight` payload: the tokenized source as a flat colored span
/// run (text + color only), sealed for the rut realm as an opaque.
pub struct PgSpans(pub Vec<SpanData>);

/// Tokenize `src` into the colored span run the editor controller renders
/// (text + color only; every other style inherits the element's defaults).
/// The join of the answer's texts is the CRLF-normalized source — spans
/// index the normalized text (rut-lexer's contract), and applying the run
/// normalizes the editor content for free.
pub fn highlight_spans(src: &str) -> Vec<SpanData> {
    let src = rut_lexer::lexer::normalize(src);
    let (toks, _diags) = rut_lexer::lexer::lex(&src);
    let mut b = Builder {
        src: &src,
        spans: Vec::new(),
        pos: 0,
    };
    let mut pos = 0usize;
    for t in &toks {
        if matches!(t.tok, Tok::Eof) {
            continue; // the tail gap after the last real token closes the run
        }
        let lo = t.span.lo as usize;
        let hi = t.span.hi as usize;
        b.gap(pos, lo);
        match &t.tok {
            Tok::FStr(fstr) => {
                // The literal colors as a string; the hole tokens (absolute
                // spans inside it) overlay their own colors.
                let mut cursor = lo;
                for part in &fstr.parts {
                    let FPart::Hole(hole) = part else {
                        continue;
                    };
                    for h in hole {
                        if matches!(h.tok, Tok::Eof) {
                            continue; // the hole's sentinel, span = the literal
                        }
                        let hlo = h.span.lo as usize;
                        let hhi = h.span.hi as usize;
                        if hlo < cursor || hhi > hi || hlo > hhi {
                            continue; // outside the literal — never happen
                        }
                        b.emit(cursor, hlo, CODE_STRING);
                        b.emit(hlo, hhi, classify(&h.tok));
                        cursor = hhi;
                    }
                }
                b.emit(cursor, hi, CODE_STRING);
            }
            _ => b.emit(lo, hi, classify(&t.tok)),
        }
        pos = hi.max(pos);
    }
    b.gap(pos, src.len());
    b.spans
}

/// One token kind → its palette color.
fn classify(tok: &Tok) -> u64 {
    match tok {
        Tok::Int(..) | Tok::Float(..) => CODE_NUMBER,
        Tok::Str(_) | Tok::RawStr(_) => CODE_STRING,
        Tok::Bool(_) => CODE_LITERAL,
        Tok::Ident(w) => classify_word(w),
        // Punctuation & operators (everything lexical left over).
        _ => CODE_OPERATOR,
    }
}

/// Keywords are `Ident`s matched by text (the parser's law). `true` /
/// `false` lex as `Tok::Bool` and `nil` stays an `Ident`, so the literal
/// trio is special-cased here. `entry` is the contextual item keyword
/// (`entry fn`); the rest of the keyword set is the parser's canonical
/// `RESERVED_KW` table (shared, not restated).
fn classify_word(w: &str) -> u64 {
    if matches!(w, "true" | "false" | "nil") {
        CODE_LITERAL
    } else if w == "entry" || rut_parser::is_reserved_kw(w) {
        CODE_KEYWORD
    } else {
        CODE_FG
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

    /// The token gap `[lo, hi)` — whitespace plus the comments the lexer
    /// skips. Comment runs color as comments, the whitespace between them
    /// stays plain.
    fn gap(&mut self, lo: usize, hi: usize) {
        if lo >= hi {
            return;
        }
        let bytes = self.src.as_bytes();
        let mut i = lo;
        let mut plain_start = lo;
        while i < hi {
            if bytes[i] == b'/' && i + 1 < hi && (bytes[i + 1] == b'/' || bytes[i + 1] == b'*') {
                if plain_start < i {
                    self.emit(plain_start, i, CODE_FG);
                }
                let end = if bytes[i + 1] == b'/' {
                    self.src[i..hi].find('\n').map_or(hi, |j| i + j)
                } else {
                    self.src[i..hi].find("*/").map_or(hi, |j| i + j + 2)
                };
                self.emit(i, end, CODE_COMMENT);
                i = end;
                plain_start = end;
            } else {
                i += 1;
            }
        }
        if plain_start < hi {
            self.emit(plain_start, hi, CODE_FG);
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
                ("f".into(), FG),
                ("(".into(), OP),
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
        assert_eq!(at("str"), FG, "primitive type names are ordinary words");
        assert_eq!(at("nil"), LITERAL);
        assert_eq!(at("->"), OP);
        // The runs join back to the source.
        let joined: String = spans.iter().map(|s| s.text.as_str()).collect();
        assert_eq!(joined, "entry fn start() -> str { return nil; }");
    }

    #[test]
    fn string_raw_string_and_fstring_holes() {
        let spans = highlight_spans("f\"x {name} y\" + r\"raw\"");
        assert_eq!(
            rgbs(&spans),
            vec![
                // The literal's prologue colors as a string (through the `{`)…
                ("f\"x {".into(), STR),
                // …the hole's tokens overlay their own colors…
                ("name".into(), FG),
                // …and the tail after the hole stays string.
                ("} y\"".into(), STR),
                (" ".into(), FG),
                ("+".into(), OP),
                (" ".into(), FG),
                ("r\"raw\"".into(), STR),
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
        let src = "use tur::{ mount };\nentry fn start() {\n    let x = f\"{x + 1}\"; // t\n}\n";
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
}
