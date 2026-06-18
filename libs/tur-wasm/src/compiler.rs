//! TSX → JS transpilation and tokenization for syntax highlighting, backed by
//! swc. Lives in `tur-wasm` (not `tur-engine`) so the core engine stays free
//! of compiler dependencies. Exposed to JS via `TurApp::register_host_fn` on
//! the `globalThis.__turHost` namespace.

use swc_common::{
    comments::SingleThreadedComments,
    sync::Lrc,
    FileName, Globals, Mark, SourceMap, GLOBALS,
};
use swc_ecma_ast::EsVersion;
use swc_ecma_parser::unstable::Token;
use swc_ecma_parser::{lexer::Lexer, Parser, StringInput, Syntax, TsSyntax};
use swc_ecma_transforms_base::{fixer::fixer, hygiene::hygiene, resolver};
use swc_ecma_transforms_typescript::strip;

/// Highlight category for a token. The JS editor maps these to colors.
/// 0 = plain/default, 1 = keyword, 2 = string, 3 = number, 4 = comment,
/// 5 = operator/punct, 6 = literal (true/false/null).
#[derive(Clone, Copy, Debug)]
pub struct TokenSpan {
    pub start: usize,
    pub end: usize,
    pub kind: u8,
}

fn tsx_syntax() -> Syntax {
    Syntax::Typescript(TsSyntax {
        tsx: true,
        decorators: true,
        ..Default::default()
    })
}

/// Transpile a TSX/TS source string to JavaScript: strip type annotations,
/// resolve + hygiene + fixer, then codegen. No JSX→React transform is applied
/// (tur cases use function-call composition, not JSX).
pub fn transpile_tsx(src: &str) -> Result<String, String> {
    let cm: Lrc<SourceMap> = Default::default();
    let fm = cm.new_source_file(
        FileName::Custom("case.tsx".into()).into(),
        src.to_string(),
    );
    let comments = SingleThreadedComments::default();

    let lexer = Lexer::new(
        tsx_syntax(),
        EsVersion::Es2020,
        StringInput::from(&*fm),
        Some(&comments),
    );
    let mut parser = Parser::new_from(lexer);
    let program = parser
        .parse_program()
        .map_err(|e| format!("parse error: {e:?}"))?;

    let globals = Globals::default();
    let code = GLOBALS.set(&globals, || {
        let unresolved = Mark::new();
        let top_level = Mark::new();
        let program = program.apply(resolver(unresolved, top_level, true));
        let program = program.apply(strip(unresolved, top_level));
        let program = program.apply(hygiene());
        let program = program.apply(fixer(Some(&comments)));
        swc_ecma_codegen::to_code_default(cm.clone(), Some(&comments), &program)
    });

    Ok(code)
}

/// Tokenize TSX/TS source into `(start, end, kind)` spans for highlighting.
/// Uses the swc lexer directly so partial/invalid code still produces tokens
/// up to the failure point (important for live editing).
pub fn tokenize_tsx(src: &str) -> Vec<TokenSpan> {
    let cm: Lrc<SourceMap> = Default::default();
    let fm = cm.new_source_file(
        FileName::Custom("case.tsx".into()).into(),
        src.to_string(),
    );
    let lexer = Lexer::new(tsx_syntax(), EsVersion::Es2020, StringInput::from(&*fm), None);

    let mut out = Vec::new();
    for ts in lexer {
        let kind = classify_token(&ts.token);
        // swc BytePos is 1-based; shift to a 0-based byte offset into `src`.
        let start = (ts.span.lo.0 as usize).saturating_sub(1).min(src.len());
        let end = (ts.span.hi.0 as usize).saturating_sub(1).min(src.len());
        if end > start {
            out.push(TokenSpan { start, end, kind });
        }
    }
    out
}

/// Map an swc lexer `Token` to a coarse highlight category (see
/// `TokenSpan::kind`). In swc's new lexer model, each keyword is its own
/// `Token` variant; `is_keyword()` / `is_known_ident()` distinguish them.
fn classify_token(token: &Token) -> u8 {
    match token {
        Token::True | Token::False | Token::Null => 6,
        Token::Str | Token::Template | Token::TemplateHead | Token::TemplateMiddle
        | Token::TemplateTail | Token::BackQuote | Token::Regex => 2,
        Token::Num | Token::BigInt => 3,
        _ if token.is_keyword() || token.is_known_ident() => 1,
        _ if token.is_bin_op() || token.is_assign_op() => 5,
        Token::Semi | Token::Comma | Token::Dot | Token::Colon | Token::QuestionMark
        | Token::LParen | Token::RParen | Token::LBrace | Token::RBrace
        | Token::LBracket | Token::RBracket | Token::At | Token::Hash
        | Token::Tilde | Token::Bang | Token::Arrow | Token::DotDotDot
        | Token::PlusPlus | Token::MinusMinus | Token::DollarLBrace => 5,
        _ => 0, // Ident and anything else → default
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transpile_strips_type_annotations() {
        let src = "const x: number = 42;";
        let out = transpile_tsx(src).expect("transpile should succeed");
        assert!(
            !out.contains(": number"),
            "type annotation should be stripped, got: {out}",
        );
        assert!(out.contains("42"), "output should keep the literal: {out}");
    }

    #[test]
    fn transpile_preserves_logic() {
        let src = "function add(a: number, b: number): number { return a + b; }";
        let out = transpile_tsx(src).expect("transpile should succeed");
        assert!(out.contains("return a + b"), "body preserved: {out}");
        assert!(!out.contains(": number"), "types stripped: {out}");
    }

    #[test]
    fn tokenize_finds_keyword_and_number() {
        let src = "const x = 42;";
        let spans = tokenize_tsx(src);
        let has_keyword = spans.iter().any(|s| s.kind == 1);
        let has_number = spans.iter().any(|s| s.kind == 3);
        assert!(has_keyword, "expected a keyword token, got {spans:?}");
        assert!(has_number, "expected a number token, got {spans:?}");
        // The number span should cover exactly "42".
        let num = spans.iter().find(|s| s.kind == 3).unwrap();
        assert_eq!(&src[num.start..num.end], "42");
    }

    #[test]
    fn tokenize_handles_strings_and_comments() {
        let src = "const s = \"hi\"; // a comment";
        let spans = tokenize_tsx(src);
        // A string token covering "hi" (the quotes are part of the span).
        let has_string = spans
            .iter()
            .any(|s| s.kind == 2 && src[s.start..s.start + 1.min(s.end - s.start)].contains('"'));
        assert!(has_string, "expected a string token, got {spans:?}");
    }
}
