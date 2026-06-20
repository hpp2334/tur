//! TSX → JS transpilation and tokenization for syntax highlighting, backed by
//! swc. Lives in `tur-wasm` (not `tur-engine`) so the core engine stays free
//! of compiler dependencies. Exposed to JS via `TurApp::register_host_fn` on
//! the `globalThis.__turHost` namespace.

use swc_common::{
    comments::SingleThreadedComments,
    sync::Lrc,
    FileName, Globals, Mark, SourceMap, Spanned, GLOBALS,
};
use swc_ecma_ast::{
    Decl, EsVersion, ModuleDecl, ModuleItem,
};
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

/// Import specifier: `{ X }` → local=X, imported=X; `{ X as Y }` → local=Y, imported=X.
#[derive(Clone, Debug)]
pub struct ImportSpecifierInfo {
    pub local: String,
    pub imported: String,
}

/// Metadata for a single top-level declaration in a module.
#[derive(Clone, Debug)]
pub enum AstNodeKind {
    /// `import { … } from "source"`
    Import {
        source: String,
        specifiers: Vec<ImportSpecifierInfo>,
    },
    /// `export const X = …` / `export function X()` / `export class X`
    ExportDecl { names: Vec<String> },
    /// `export default X`
    ExportDefault,
    /// `export { X, Y }` or `export { X } from "..."`
    ExportNamed { names: Vec<String> },
    /// `export * from "..."`
    ExportAll,
    /// `export interface X` / `export type X` — type-only, no runtime value
    ExportType { names: Vec<String> },
    /// Any other statement (not an import or export)
    Statement,
}

#[derive(Clone, Debug)]
pub struct AstNode {
    pub kind: AstNodeKind,
    /// Source text of this node, extracted from the original string.
    /// The JS side uses this instead of position-based slicing.
    pub text: String,
    /// For export nodes: the declaration text WITHOUT the `export`/`export
    /// default` keyword, extracted from the inner declaration's span.  `None`
    /// for non-export nodes (import, statement).  This lets the JS rewriter
    /// avoid any regex — it just uses `node.body` directly.
    pub body: Option<String>,
}

/// Parse TS/TSX source and return structured metadata for each top-level
/// declaration. Does NOT apply transforms (strip, resolver, etc.) — this is a
/// lightweight parse that walks the raw AST.
pub fn generate_ast(src: &str) -> Result<Vec<AstNode>, String> {
    let cm: Lrc<SourceMap> = Default::default();
    let fm = cm.new_source_file(
        FileName::Custom("case.tsx".into()).into(),
        src.to_string(),
    );
    let lexer = Lexer::new(
        tsx_syntax(),
        EsVersion::Es2020,
        StringInput::from(&*fm),
        None,
    );
    let mut parser = Parser::new_from(lexer);
    let program = parser
        .parse_program()
        .map_err(|e| format!("parse error: {e:?}"))?;

    let module = match program {
        swc_ecma_ast::Program::Module(m) => m,
        _ => return Ok(vec![]),
    };

    let mut nodes = Vec::new();
    for item in &module.body {
        let span = item.span();
        let text = extract_span_text(src, span);

        let (kind, body) = match item {
            ModuleItem::ModuleDecl(ModuleDecl::Import(import)) => {
                let source = import.src.value.to_atom_lossy().as_str().to_string();
                let specifiers = import
                    .specifiers
                    .iter()
                    .map(|s| match s {
                        swc_ecma_ast::ImportSpecifier::Named(n) => ImportSpecifierInfo {
                            local: n.local.sym.as_str().to_string(),
                            imported: n.imported
                                .as_ref()
                                .map(|i| match i {
                                    swc_ecma_ast::ModuleExportName::Ident(id) => id.sym.as_str().to_string(),
                                    swc_ecma_ast::ModuleExportName::Str(s) => s.value.to_atom_lossy().as_str().to_string(),
                                })
                                .unwrap_or_else(|| n.local.sym.as_str().to_string()),
                        },
                        swc_ecma_ast::ImportSpecifier::Default(d) => ImportSpecifierInfo {
                            local: d.local.sym.as_str().to_string(),
                            imported: "default".to_string(),
                        },
                        swc_ecma_ast::ImportSpecifier::Namespace(ns) => ImportSpecifierInfo {
                            local: ns.local.sym.as_str().to_string(),
                            imported: "*".to_string(),
                        },
                    })
                    .collect();
                (AstNodeKind::Import { source, specifiers }, None)
            }

            ModuleItem::ModuleDecl(ModuleDecl::ExportDecl(export)) => {
                let body_text = extract_span_text(src, export.decl.span());
                let names = extract_decl_names(&export.decl);
                if names.is_empty() {
                    (
                        AstNodeKind::ExportType {
                            names: extract_type_decl_names(&export.decl),
                        },
                        Some(body_text),
                    )
                } else {
                    (AstNodeKind::ExportDecl { names }, Some(body_text))
                }
            }

            ModuleItem::ModuleDecl(ModuleDecl::ExportDefaultDecl(export)) => {
                let body_text = extract_span_text(src, export.decl.span());
                (AstNodeKind::ExportDefault, Some(body_text))
            }
            ModuleItem::ModuleDecl(ModuleDecl::ExportDefaultExpr(export)) => {
                let body_text = extract_span_text(src, export.expr.span());
                (AstNodeKind::ExportDefault, Some(body_text))
            }

            ModuleItem::ModuleDecl(ModuleDecl::ExportNamed(export)) => {
                let names = export
                    .specifiers
                    .iter()
                    .map(|s| match s {
                        swc_ecma_ast::ExportSpecifier::Named(n) => n
                            .exported
                            .as_ref()
                            .map(|e| match e {
                                swc_ecma_ast::ModuleExportName::Ident(id) => id.sym.as_str().to_string(),
                                swc_ecma_ast::ModuleExportName::Str(s) => s.value.to_atom_lossy().as_str().to_string(),
                            })
                            .unwrap_or_else(|| match &n.orig {
                                swc_ecma_ast::ModuleExportName::Ident(id) => id.sym.as_str().to_string(),
                                swc_ecma_ast::ModuleExportName::Str(s) => s.value.to_atom_lossy().as_str().to_string(),
                            }),
                        swc_ecma_ast::ExportSpecifier::Default(_) => "default".to_string(),
                        swc_ecma_ast::ExportSpecifier::Namespace(_) => "*".to_string(),
                    })
                    .collect();
                (AstNodeKind::ExportNamed { names }, None)
            }

            ModuleItem::ModuleDecl(ModuleDecl::ExportAll(_)) => {
                (AstNodeKind::ExportAll, None)
            }

            _ => (AstNodeKind::Statement, None),
        };

        nodes.push(AstNode { kind, text, body });
    }

    Ok(nodes)
}

/// Safely extract a substring from `src` using a swc `Span`.  swc `BytePos` is
/// 1-based, so we shift to 0-based.  Bounds-checked to avoid panics on invalid
/// spans.
fn extract_span_text(src: &str, span: swc_common::Span) -> String {
    let start = span.lo.0.saturating_sub(1) as usize;
    let end = span.hi.0.saturating_sub(1) as usize;
    if start < end && end <= src.len() {
        src[start..end].to_string()
    } else {
        String::new()
    }
}

/// Extract runtime-exported names from a declaration (var, fn, class).
/// Returns empty for type-only declarations (interface, type alias).
fn extract_decl_names(decl: &Decl) -> Vec<String> {
    match decl {
        Decl::Var(var) => var
            .decls
            .iter()
            .filter_map(|d| match &d.name {
                swc_ecma_ast::Pat::Ident(id) => Some(id.id.sym.as_str().to_string()),
                _ => None,
            })
            .collect(),
        Decl::Fn(f) => vec![f.ident.sym.as_str().to_string()],
        Decl::Class(c) => vec![c.ident.sym.as_str().to_string()],
        _ => vec![],
    }
}

/// Extract names from type-only declarations (interface, type alias).
fn extract_type_decl_names(decl: &Decl) -> Vec<String> {
    match decl {
        Decl::TsInterface(i) => vec![i.id.sym.as_str().to_string()],
        Decl::TsTypeAlias(a) => vec![a.id.sym.as_str().to_string()],
        Decl::TsEnum(e) => vec![e.id.sym.as_str().to_string()],
        Decl::TsModule(m) => match &m.id {
            swc_ecma_ast::TsModuleName::Ident(id) => vec![id.sym.as_str().to_string()],
            swc_ecma_ast::TsModuleName::Str(s) => vec![s.value.to_atom_lossy().as_str().to_string()],
        },
        _ => vec![],
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

    #[test]
    fn generate_ast_finds_export_function() {
        let src = "export function openAddModal(ctx) { ctx.set(addOpen$, true); }";
        let nodes = generate_ast(src).expect("parse should succeed");
        assert_eq!(nodes.len(), 1);
        match &nodes[0].kind {
            AstNodeKind::ExportDecl { names } => {
                assert_eq!(names, &["openAddModal"]);
            }
            other => panic!("expected ExportDecl, got {other:?}"),
        }
        assert!(
            nodes[0].text.starts_with("export function openAddModal"),
            "text was: {}",
            nodes[0].text
        );
        let body = nodes[0].body.as_ref().expect("body should be Some");
        assert!(
            body.starts_with("function openAddModal"),
            "body should not have 'export' prefix, was: {body}",
        );
        assert!(!body.contains("export"), "body must not contain 'export': {body}");
    }

    #[test]
    fn generate_ast_finds_export_const() {
        let src = "export const tasks$ = source([]);";
        let nodes = generate_ast(src).expect("parse should succeed");
        assert_eq!(nodes.len(), 1);
        match &nodes[0].kind {
            AstNodeKind::ExportDecl { names } => {
                assert_eq!(names, &["tasks$"]);
            }
            other => panic!("expected ExportDecl, got {other:?}"),
        }
        let body = nodes[0].body.as_ref().expect("body should be Some");
        assert!(
            body.starts_with("const tasks$"),
            "body should start with 'const tasks$', was: {body}",
        );
    }

    #[test]
    fn generate_ast_finds_import() {
        let src = "import { Container, Text } from \"@tur/edgy\";";
        let nodes = generate_ast(src).expect("parse should succeed");
        assert_eq!(nodes.len(), 1);
        match &nodes[0].kind {
            AstNodeKind::Import { source, specifiers } => {
                assert_eq!(source, "@tur/edgy");
                assert_eq!(specifiers.len(), 2);
                assert_eq!(specifiers[0].local, "Container");
                assert_eq!(specifiers[1].local, "Text");
            }
            other => panic!("expected Import, got {other:?}"),
        }
        assert!(nodes[0].body.is_none(), "import body should be None");
    }

    #[test]
    fn generate_ast_finds_export_default() {
        let src = "export default component(() => {});";
        let nodes = generate_ast(src).expect("parse should succeed");
        assert_eq!(nodes.len(), 1);
        match &nodes[0].kind {
            AstNodeKind::ExportDefault => {}
            other => panic!("expected ExportDefault, got {other:?}"),
        }
        assert!(
            nodes[0].text.starts_with("export default"),
            "text was: {}",
            nodes[0].text
        );
        let body = nodes[0].body.as_ref().expect("body should be Some");
        assert!(
            body.starts_with("component"),
            "body should start with 'component', was: {body}",
        );
        assert!(
            !body.contains("export"),
            "body must not contain 'export': {body}",
        );
    }

    #[test]
    fn generate_ast_strips_type_export() {
        let src = "export interface Task { title: string; }";
        let nodes = generate_ast(src).expect("parse should succeed");
        assert_eq!(nodes.len(), 1);
        match &nodes[0].kind {
            AstNodeKind::ExportType { names } => {
                assert_eq!(names, &["Task"]);
            }
            other => panic!("expected ExportType, got {other:?}"),
        }
    }
}
