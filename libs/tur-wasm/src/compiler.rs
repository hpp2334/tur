//! TSX → JS transpilation and tokenization for syntax highlighting, backed by
//! swc. Lives in `tur-wasm` (not `tur-engine`) so the core engine stays free
//! of compiler dependencies. Exposed to JS via `TurApp::register_host_fn` on
//! the `globalThis.__turHost` namespace.

use swc_common::{
    comments::SingleThreadedComments,
    input::StringInput,
    sync::Lrc,
    BytePos, FileName, Globals, Mark, SourceMap, Span, Spanned, GLOBALS,
};
use swc_ecma_ast::{Decl, EsVersion, ModuleDecl, ModuleItem};
use swc_ecma_parser::unstable::Token;
use swc_ecma_parser::{Lexer, Parser, Syntax, TsSyntax};
use swc_ecma_transforms_base::{fixer::fixer, hygiene::hygiene, resolver};
use swc_ecma_transforms_typescript::strip;
use swc_ecma_visit::{Visit, VisitWith};

/// Highlight category for a token. The JS editor maps these to colors.
/// Lexical kinds (from the lexer): 0 = plain/default, 1 = keyword,
/// 2 = string, 3 = number, 4 = comment, 5 = operator/punct,
/// 6 = literal (true/false/null).
/// Semantic kinds (from the AST overlay): 7 = declaration name (fn/view/
/// const binding/imported binding/call callee), 8 = JSX tag name, 9 = JSX
/// attribute name, 10 = type name, 11 = property (object-literal key or member
/// access `.prop`).
#[derive(Clone, Copy, Debug)]
pub struct TokenSpan {
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) kind: u8,
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

/// Map every byte offset of `src` to the count of UTF-16 code units preceding
/// it: `map[byte_off] = utf16_off`. swc spans are byte offsets, but the JS
/// editor slices with UTF-16 indices (`String.prototype.slice`); for any
/// non-ASCII char (e.g. `—`, `·`) the two diverge, so we translate here.
fn byte_to_utf16_map(src: &str) -> Vec<usize> {
    let mut map = vec![0usize; src.len() + 1];
    let mut utf16 = 0usize;
    let mut i = 0usize;
    while i < src.len() {
        let ch = src[i..].chars().next().unwrap();
        let len = ch.len_utf8();
        map[i..i + len].fill(utf16);
        utf16 += ch.len_utf16();
        i += len;
    }
    map[src.len()] = utf16;
    map
}

/// Convert a 1-based swc `Span` to a clamped 0-based byte range into `src`.
/// Empty ranges (`start == end`) return `None`. Mirrors the offset logic in
/// `tokenize_tsx` / `extract_span_text`.
fn span_to_range(span: Span, src_len: usize) -> Option<(usize, usize)> {
    let start = (span.lo.0 as usize).saturating_sub(1).min(src_len);
    let end = (span.hi.0 as usize).saturating_sub(1).min(src_len);
    (start < end).then_some((start, end))
}

/// Span of a JSX element/attribute name, covering all variants (plain ident,
/// namespaced `a:b`, member `a.b`).
fn jsx_name_span(name: &swc_ecma_ast::JSXElementName) -> Option<Span> {
    use swc_ecma_ast::JSXElementName;
    match name {
        JSXElementName::Ident(id) => Some(id.span),
        JSXElementName::JSXNamespacedName(ns) => Some(ns.span),
        JSXElementName::JSXMemberExpr(m) => Some(m.span),
    }
}

/// AST-driven overlay that reclassifies identifier ranges into semantic
/// highlight kinds. Walked over a parsed `Program`; pushes `(start, end, kind)`
/// entries which are later matched against lexical token spans by containment.
struct HighlightOverlay {
    src_len: usize,
    ranges: Vec<(usize, usize, u8)>,
}

impl HighlightOverlay {
    fn push(&mut self, span: Span, kind: u8) {
        if let Some((s, e)) = span_to_range(span, self.src_len) {
            self.ranges.push((s, e, kind));
        }
    }
}

impl Visit for HighlightOverlay {
    fn visit_fn_decl(&mut self, n: &swc_ecma_ast::FnDecl) {
        self.push(n.ident.span, 7);
        n.visit_children_with(self);
    }

    fn visit_var_declarator(&mut self, n: &swc_ecma_ast::VarDeclarator) {
        if let swc_ecma_ast::Pat::Ident(b) = &n.name {
            self.push(b.span, 7);
        }
        n.visit_children_with(self);
    }

    fn visit_import_decl(&mut self, n: &swc_ecma_ast::ImportDecl) {
        // Color each imported binding name as a declaration (kind 7).
        use swc_ecma_ast::ImportSpecifier;
        for spec in &n.specifiers {
            let span = match spec {
                ImportSpecifier::Named(s) => Some(s.local.span),
                ImportSpecifier::Default(d) => Some(d.local.span),
                ImportSpecifier::Namespace(ns) => Some(ns.local.span),
            };
            if let Some(s) = span {
                self.push(s, 7);
            }
        }
        n.visit_children_with(self);
    }

    fn visit_call_expr(&mut self, n: &swc_ecma_ast::CallExpr) {
        // Color the callee identifier of a plain call (`foo(...)`) as a
        // function/view name (kind 7). Member callees (`obj.m()`) are
        // handled by `visit_member_expr` (the `.m` is a property, kind 11).
        if let swc_ecma_ast::Callee::Expr(e) = &n.callee {
            if let swc_ecma_ast::Expr::Ident(id) = e.as_ref() {
                self.push(id.span, 7);
            }
        }
        n.visit_children_with(self);
    }

    fn visit_key_value_prop(&mut self, n: &swc_ecma_ast::KeyValueProp) {
        // Object-literal key (`{ child: … }`) → property (kind 11).
        if let swc_ecma_ast::PropName::Ident(id) = &n.key {
            self.push(id.span, 11);
        }
        n.visit_children_with(self);
    }

    fn visit_member_expr(&mut self, n: &swc_ecma_ast::MemberExpr) {
        // `.prop` is a property (kind 11); the object identifier
        // (`Color` in `Color.hex`, `CrossAxisAlignment` in `.Center`) is a
        // value/type name → kind 7, matching its import declaration.
        if let swc_ecma_ast::MemberProp::Ident(id) = &n.prop {
            self.push(id.span, 11);
        }
        if let swc_ecma_ast::Expr::Ident(id) = n.obj.as_ref() {
            self.push(id.span, 7);
        }
        n.visit_children_with(self);
    }

    fn visit_jsx_opening_element(&mut self, n: &swc_ecma_ast::JSXOpeningElement) {
        if let Some(span) = jsx_name_span(&n.name) {
            self.push(span, 8);
        }
        n.visit_children_with(self);
    }

    fn visit_jsx_closing_element(&mut self, n: &swc_ecma_ast::JSXClosingElement) {
        if let Some(span) = jsx_name_span(&n.name) {
            self.push(span, 8);
        }
        n.visit_children_with(self);
    }

    fn visit_jsx_attr(&mut self, n: &swc_ecma_ast::JSXAttr) {
        if let swc_ecma_ast::JSXAttrName::Ident(id) = &n.name {
            self.push(id.span, 9);
        }
        n.visit_children_with(self);
    }

    fn visit_ts_type_ref(&mut self, n: &swc_ecma_ast::TsTypeRef) {
        let span = match &n.type_name {
            swc_ecma_ast::TsEntityName::Ident(id) => Some(id.span),
            swc_ecma_ast::TsEntityName::TsQualifiedName(q) => Some(q.span),
        };
        if let Some(s) = span {
            self.push(s, 10);
        }
        n.visit_children_with(self);
    }

    fn visit_ts_interface_decl(&mut self, n: &swc_ecma_ast::TsInterfaceDecl) {
        self.push(n.id.span, 10);
        n.visit_children_with(self);
    }

    fn visit_ts_type_alias_decl(&mut self, n: &swc_ecma_ast::TsTypeAliasDecl) {
        self.push(n.id.span, 10);
        n.visit_children_with(self);
    }
}

/// A `Visit` impl that collects the byte ranges of every template literal
/// (`` `…` ``, including the `${ expr }` parts). We need these because the swc
/// lexer, iterated standalone, mishandles template literals — after `${ … }`
/// it loses template context and swallows the rest of the file as one bogus
/// token. By carving template literals out of the source before tokenizing,
/// each remaining segment has no templates and lexes correctly.
struct TemplateSpanCollector {
    src_len: usize,
    spans: Vec<(usize, usize)>,
}

impl Visit for TemplateSpanCollector {
    fn visit_tpl(&mut self, n: &swc_ecma_ast::Tpl) {
        if let Some((s, e)) = span_to_range(n.span, self.src_len) {
            self.spans.push((s, e));
        }
        // Do NOT recurse into `quasis`/`expressions`: we treat the whole
        // template (including nested templates) as one opaque region. Nested
        // template spans are merged away by the caller anyway.
    }
}

/// Lex a template-free segment of source (relative to `offset`) using the
/// standalone lexer, pushing absolute byte-offset spans into `out`.
fn lex_segment(segment: &str, offset: usize, src_len: usize, out: &mut Vec<TokenSpan>) {
    if segment.is_empty() {
        return;
    }
    let input = StringInput::new(segment, BytePos(1), BytePos(1 + segment.len() as u32));
    for ts in Lexer::new(tsx_syntax(), EsVersion::Es2020, input, None) {
        let kind = classify_token(&ts.token);
        let rel_start = (ts.span.lo.0 as usize).saturating_sub(1).min(segment.len());
        let rel_end = (ts.span.hi.0 as usize).saturating_sub(1).min(segment.len());
        let start = offset + rel_start;
        let end = offset + rel_end;
        if end > start && end <= src_len {
            out.push(TokenSpan { start, end, kind });
        }
    }
}

/// Produce highlight spans for TSX/TS source. Template literals (which the
/// standalone lexer can't tokenize correctly) are carved out using their AST
/// spans and emitted as a single string span; the surrounding segments lex
/// cleanly. The result is then refined by AST-derived semantic kinds
/// (declaration/import/call names, JSX tags/attributes, type names, object
/// keys, member props) and comment spans. If the source fails to parse
/// (e.g. while the user is mid-keystroke) the whole source is lexed as one
/// segment — highlighting degrades gracefully and never throws.
pub fn highlight_tsx(src: &str) -> Vec<TokenSpan> {
    let src_len = src.len();

    // Parse for the AST (semantic overlay + template spans + comments).
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
    let parse_result = parser.parse_program();

    // Collect + merge template-literal byte ranges.
    let mut templates: Vec<(usize, usize)> = Vec::new();
    if let Ok(program) = &parse_result {
        let mut c = TemplateSpanCollector { src_len, spans: Vec::new() };
        program.visit_with(&mut c);
        templates = c.spans;
    }
    templates.sort();
    let mut merged: Vec<(usize, usize)> = Vec::new();
    for (s, e) in templates {
        match merged.last_mut() {
            Some(last) if s <= last.1 => last.1 = last.1.max(e),
            _ => merged.push((s, e)),
        }
    }

    // Lex each template-free segment; emit one string span per template.
    let mut out: Vec<TokenSpan> = Vec::new();
    let mut cursor = 0usize;
    for &(t_lo, t_hi) in &merged {
        if t_lo > cursor {
            lex_segment(&src[cursor..t_lo], cursor, src_len, &mut out);
        }
        out.push(TokenSpan { start: t_lo, end: t_hi, kind: 2 });
        cursor = t_hi;
    }
    if cursor < src_len {
        lex_segment(&src[cursor..], cursor, src_len, &mut out);
    }
    // If nothing parsed at all, make sure we still produced something.
    if out.is_empty() && src_len > 0 {
        lex_segment(src, 0, src_len, &mut out);
    }

    // AST overlay (semantic) + comments, only when parse succeeded.
    if let Ok(program) = parse_result {
        let mut overlay = HighlightOverlay { src_len, ranges: Vec::new() };
        program.visit_with(&mut overlay);
        if !overlay.ranges.is_empty() {
            for tok in &mut out {
                // Don't recolor tokens emitted as template strings.
                if tok.kind == 2 && template_contains(&merged, tok) {
                    continue;
                }
                for &(s, e, kind) in &overlay.ranges {
                    if s <= tok.start && tok.end <= e {
                        tok.kind = kind;
                        break;
                    }
                }
            }
        }

        let (leading, trailing) = comments.take_all();
        for map in [leading, trailing] {
            for (_pos, cmts) in map.borrow().iter() {
                for c in cmts {
                    if let Some((s, e)) = span_to_range(c.span, src_len) {
                        out.push(TokenSpan { start: s, end: e, kind: 4 });
                    }
                }
            }
        }
    }

    out.sort_by_key(|t| t.start);
    out.dedup_by(|a, b| a.start == b.start && a.end == b.end);

    // Translate byte offsets → UTF-16 code-unit offsets (JS slices by UTF-16).
    let b2u = byte_to_utf16_map(src);
    for t in &mut out {
        t.start = b2u[t.start];
        t.end = b2u[t.end];
    }
    out
}

/// True if `tok`'s range is inside one of the (merged) template regions.
fn template_contains(templates: &[(usize, usize)], tok: &TokenSpan) -> bool {
    templates.iter().any(|(s, e)| *s <= tok.start && tok.end <= *e)
}

/// Import specifier: `{ X }` → local=X, imported=X; `{ X as Y }` → local=Y, imported=X.
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct ImportSpecifierInfo {
    pub(crate) local: String,
    pub(crate) imported: String,
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
#[allow(dead_code)]
pub struct AstNode {
    pub(crate) kind: AstNodeKind,
    /// Source text of this node, extracted from the original string.
    /// The JS side uses this instead of position-based slicing.
    pub(crate) text: String,
    /// For export nodes: the declaration text WITHOUT the `export`/`export
    /// default` keyword, extracted from the inner declaration's span.  `None`
    /// for non-export nodes (import, statement).  This lets the JS rewriter
    /// avoid any regex — it just uses `node.body` directly.
    pub(crate) body: Option<String>,
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
        let src = "import { Container, Text } from \"tur:std\";";
        let nodes = generate_ast(src).expect("parse should succeed");
        assert_eq!(nodes.len(), 1);
        match &nodes[0].kind {
            AstNodeKind::Import { source, specifiers } => {
                assert_eq!(source, "tur:std");
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
        let src = "export default view(() => {});";
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
            body.starts_with("view"),
            "body should start with 'view', was: {body}",
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

    #[test]
    fn highlight_colors_comments() {
        let src = "const x = 1; // hello";
        let spans = highlight_tsx(src);
        // A kind-4 (comment) span covering the "// hello" text exactly.
        let cmt = spans
            .iter()
            .find(|s| s.kind == 4)
            .unwrap_or_else(|| panic!("expected a comment span, got {spans:?}"));
        assert_eq!(&src[cmt.start..cmt.end], "// hello");
    }

    #[test]
    fn highlight_colors_block_comments() {
        let src = "/* c */ const x = 1;";
        let spans = highlight_tsx(src);
        let cmt = spans.iter().find(|s| s.kind == 4).expect("block comment");
        assert_eq!(&src[cmt.start..cmt.end], "/* c */");
    }

    #[test]
    fn highlight_recolors_fn_decl_name() {
        let src = "function greet() { return 1; }";
        let spans = highlight_tsx(src);
        // The `greet` identifier token should be reclassified to kind 7.
        let greet = spans
            .iter()
            .find(|s| &src[s.start..s.end] == "greet")
            .unwrap_or_else(|| panic!("expected a `greet` span, got {spans:?}"));
        assert_eq!(greet.kind, 7, "fn name should be decl (7), got {spans:?}");
    }

    #[test]
    fn highlight_recolors_const_binding() {
        let src = "const tasks$ = source([]);";
        let spans = highlight_tsx(src);
        let name = spans
            .iter()
            .find(|s| &src[s.start..s.end] == "tasks$")
            .expect("tasks$ span");
        assert_eq!(name.kind, 7, "const binding should be decl (7)");
    }

    #[test]
    fn highlight_recolors_jsx_tag_and_attr() {
        let src = "const el = <Container color=\"red\" />;";
        let spans = highlight_tsx(src);
        let tag = spans
            .iter()
            .find(|s| &src[s.start..s.end] == "Container")
            .unwrap_or_else(|| panic!("expected a Container tag span, got {spans:?}"));
        assert_eq!(tag.kind, 8, "JSX tag should be kind 8");
        let attr = spans
            .iter()
            .find(|s| &src[s.start..s.end] == "color")
            .unwrap_or_else(|| panic!("expected a color attr span, got {spans:?}"));
        assert_eq!(attr.kind, 9, "JSX attr should be kind 9");
    }

    #[test]
    fn highlight_recolors_type_names() {
        // `Task` is both a declared interface name (kind 10) and referenced
        // in the annotation `: Task` (kind 10).
        let src = "interface Task {} const t: Task = null as any;";
        let spans = highlight_tsx(src);
        let task_spans: Vec<_> = spans.iter().filter(|s| &src[s.start..s.end] == "Task").collect();
        assert!(
            !task_spans.is_empty(),
            "expected Task spans, got {spans:?}",
        );
        assert!(
            task_spans.iter().all(|s| s.kind == 10),
            "all Task spans should be type (10), got {task_spans:?}",
        );
    }

    #[test]
    fn highlight_falls_back_on_parse_error() {
        // Unbalanced — won't parse. Must still return lexical tokens (no panic).
        let src = "const x = {{{";
        let spans = highlight_tsx(src);
        assert!(!spans.is_empty(), "should still return lexical tokens");
        // The `const` keyword is lexical kind 1 even without an AST.
        assert!(spans.iter().any(|s| s.kind == 1), "keyword kept, got {spans:?}");
    }

    #[test]
    fn highlight_plain_reference_stays_default() {
        // `make` declared (kind 7). A non-call reference (`const x = make`)
        // stays default (kind 0); only a call *callee* (`make(...)`) is colored.
        let src = "const make = 1; const x = make;";
        let spans = highlight_tsx(src);
        let kind_of_nth = |name: &str, n: usize| {
            spans
                .iter()
                .filter(|s| &src[s.start..s.end] == name)
                .nth(n)
                .map(|s| s.kind)
        };
        assert_eq!(kind_of_nth("make", 0), Some(7), "declared make → 7");
        assert_eq!(kind_of_nth("make", 1), Some(0), "plain reference → 0");
    }

    #[test]
    fn highlight_offsets_are_utf16_after_non_ascii() {
        // An em-dash (`—`: 3 bytes, 1 UTF-16 unit) in a comment precedes the
        // binding `num`. swc gives byte offsets; JS slices by UTF-16, so the
        // returned span must use UTF-16 indices or the editor grabs the wrong
        // text. `num` is at byte 20 but UTF-16 index 18.
        let src = "// note — x\nconst num = 1;";
        let spans = highlight_tsx(src);
        let units: Vec<char> = src.chars().collect();
        let num = spans
            .iter()
            .find(|s| units[s.start..s.end].iter().collect::<String>() == "num")
            .expect("a span should reconstruct `num`");
        assert_eq!(num.start, 18, "must be UTF-16 index, not byte offset 20");
        assert_eq!(num.end, 21);
        assert_ne!(num.start, 20, "regression: returned raw byte offset");
    }

    #[test]
    fn highlight_colors_import_bindings() {
        let src = "import { Column, derive } from \"x\";";
        let spans = highlight_tsx(src);
        let kind_of = |name: &str| {
            spans
                .iter()
                .find(|s| &src[s.start..s.end] == name)
                .map(|s| s.kind)
        };
        assert_eq!(kind_of("Column"), Some(7), "imported binding → decl");
        assert_eq!(kind_of("derive"), Some(7), "imported binding → decl");
    }

    #[test]
    fn highlight_colors_call_callee() {
        // `make` declared and called; `inner` a nested callee; `Color.hex` is a
        // member callee (the `.hex` is a property, not a plain callee).
        let src = "const make = () => inner(1); make(2);";
        let spans = highlight_tsx(src);
        // First `make` (decl) and second `make` (call callee) both kind 7.
        let makes: Vec<_> = spans.iter().filter(|s| &src[s.start..s.end] == "make").collect();
        assert_eq!(makes.len(), 2);
        assert!(makes.iter().all(|s| s.kind == 7), "decl + callee both 7");
        // `inner` is a call callee → 7.
        let inner = spans.iter().find(|s| &src[s.start..s.end] == "inner").unwrap();
        assert_eq!(inner.kind, 7);
    }

    #[test]
    fn highlight_colors_object_keys_and_member_props() {
        let src = "const o = { child: 1 }; o.child;";
        let spans = highlight_tsx(src);
        // Both `child` tokens (object-literal key + member `.child`) → 11.
        let childs: Vec<_> = spans.iter().filter(|s| &src[s.start..s.end] == "child").collect();
        assert_eq!(childs.len(), 2, "expected two `child` tokens: {spans:?}");
        assert!(
            childs.iter().all(|s| s.kind == 11),
            "both `child` tokens should be property (11)",
        );
        // `o` appears as a decl (kind 7) and as a member-obj reference (now
        // also kind 7, matching imported/class names like `Color.hex`).
        let o_refs: Vec<_> = spans.iter().filter(|s| &src[s.start..s.end] == "o").collect();
        assert_eq!(o_refs.len(), 2);
        assert_eq!(o_refs[0].kind, 7, "declared `o` → 7");
        assert_eq!(o_refs[1].kind, 7, "member-obj `o` reference → 7");
    }

    #[test]
    fn highlight_colors_member_access_object() {
        // `Color.hex(…)` and `CrossAxisAlignment.Center` — the object ident
        // (`Color`, `CrossAxisAlignment`) is colored kind 7 (value/class name)
        // and the `.prop` (`hex`, `Center`) is kind 11 (property).
        let src = "Color.hex(1); CrossAxisAlignment.Center;";
        let spans = highlight_tsx(src);
        let kind_of = |name: &str| {
            spans
                .iter()
                .find(|s| &src[s.start..s.end] == name)
                .map(|s| s.kind)
        };
        assert_eq!(kind_of("Color"), Some(7));
        assert_eq!(kind_of("hex"), Some(11));
        assert_eq!(kind_of("CrossAxisAlignment"), Some(7));
        assert_eq!(kind_of("Center"), Some(11));
    }

    #[test]
    fn highlight_template_literal_does_not_break_after() {
        // The swc lexer, iterated standalone, swallows everything after a
        // template literal's `${ … }` as one bogus token. highlight_tsx must
        // carve the template out (emit it as a string span) so the code after
        // it (`const b = 42`) is still tokenized — `42` must be a number.
        let src = "const a = `x ${1} y`; const b = 42;";
        let spans = highlight_tsx(src);
        let has_42_as_number = spans
            .iter()
            .any(|s| s.kind == 3 && &src[s.start..s.end] == "42");
        assert!(
            has_42_as_number,
            "highlighting must continue past the template literal: {spans:?}",
        );
        // And the template literal itself is a string span.
        assert!(
            spans.iter().any(|s| s.kind == 2 && src[s.start..s.end].contains('`')),
            "template literal should be a string span: {spans:?}",
        );
    }
}
