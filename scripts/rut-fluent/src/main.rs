//! rut-fluent — the kit fluent-construction rewriter (phase 2 of
//! `kit-fluent-construction`; kept for future case normalization).
//!
//! Rewrites authored rut sources to the kit's fluent construction idiom:
//!
//! ```text
//! let mut x = T.builder();      →   let x = T().p(..).c(..);
//! x.p(..);
//! x.c(..);
//! ```
//!
//! Transform rules (conservative — kit row calls have side effects):
//!   1. fold only CONTIGUOUS statement runs targeting one binding;
//!   2. never reorder across an interleaved statement (any other `let`,
//!      side-effect call, control flow, or comment ends the run — that
//!      binding gets partial chaining only);
//!   3. a `let y = B.builder(..)` unit sandwiched inside the run whose
//!      binding is used exactly once as `x.m(y.build())` — immediately
//!      after the unit — inlines into the chain at that position;
//!   4. `let mut` → `let` when no mutating use remains (`build()` — the
//!      kit's only non-`mut self` method — does not count);
//!   5. `.builder(` → `(` at every remaining (chained/inline) call site.
//!
//! Long chains (> 100 columns flat) break one segment per line at the
//! outer chain's `.method(` boundaries.
//!
//! The rewrite is AST-level (rut-parser) with token-exact edit ranges
//! (rut-lexer — AST statement spans bleed into trailing trivia, tokens
//! do not) and text-splicing, so untouched code stays byte-identical.
//! Every rewritten source is re-parsed (0 diagnostics required) before
//! the file is written; a source that fails any check is left untouched
//! and reported.
//!
//! Usage:
//!
//! ```sh
//! cargo run --manifest-path scripts/rut-fluent/Cargo.toml -- FILES...
//! ```
//!
//! `.rut` files are rewritten in place. `.rs` files have their rut
//! fixtures (raw-string literals that parse as rut modules) rewritten in
//! place. Use `git diff` to review.

use rut_ast::ast::{
    ArmKind, Ast, ElseBranch, ExprKind, FPartAst, ItemKind, Kind, MemberKind, NodeHandle, NodeId,
    PatKind, StmtKind, AnyExpr, AnyStmt,
};
use rut_lexer::span::Span;
use rut_lexer::token::{Tok, Token};
use rut_parser::Mode;

// ---------------------------------------------------------------- state

struct Edit {
    lo: usize,
    hi: usize,
    text: String,
}

#[derive(Default)]
struct Stats {
    folds: usize,
    units: usize,
    removals: usize,
    mut_kept: usize,
}

enum Elem {
    /// a plain `x.m(..);` statement folded into the chain
    Stmt {
        stmt_lo: usize,
        /// receiver span to delete (`x`)
        recv_hi: usize,
        /// hi of the call's closing paren — the element's end
        close_hi: usize,
    },
    /// a `let y = B.builder(..); y.*(..); x.m(y.build());` unit inlined
    /// into the chain (rule 3)
    Unit {
        /// delete [unit_lo .. unit_hi) — the unit's let + its statements
        /// + the gap up to the consumer statement
        unit_lo: usize,
        unit_hi: usize,
        /// the consumer `x.m(..)`'s receiver deletion
        cons_lo: usize,
        cons_hi_recv: usize,
        cons_close_hi: usize,
        /// replace this range (`y.build()`) with the inlined chain
        build_lo: usize,
        build_hi: usize,
        chain: String,
    },
}

// ------------------------------------------------------------------ main

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: rut-fluent <file.rut|file.rs>...");
        std::process::exit(2);
    }
    let mut total = Stats::default();
    for path in &args {
        if let Err(e) = process_file(path, &mut total) {
            eprintln!("{path}: {e}");
        }
    }
    println!(
        "total: {} folds ({} `mut` kept, {} units inlined), {} `.builder` removals",
        total.folds, total.mut_kept, total.units, total.removals
    );
}

fn process_file(path: &str, total: &mut Stats) -> Result<(), String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let (new_text, stats) = if path.ends_with(".rs") {
        transform_rs(&text).map_err(|(m, _)| m)?
    } else {
        transform_source(&text).map_err(|(m, _)| m)?
    };
    if new_text != text {
        std::fs::write(path, new_text).map_err(|e| e.to_string())?;
    }
    println!(
        "{path}: {} folds ({} `mut` kept, {} units), {} removals",
        stats.folds, stats.mut_kept, stats.units, stats.removals
    );
    total.folds += stats.folds;
    total.units += stats.units;
    total.removals += stats.removals;
    total.mut_kept += stats.mut_kept;
    Ok(())
}

// ------------------------------------------------------- .rs raw strings

/// Transform every raw-string literal in a Rust file whose content parses
/// as a clean rut module and contains `.builder(`.
fn transform_rs(text: &str) -> Result<(String, Stats), (String, Option<usize>)> {
    let bytes = text.as_bytes();
    let mut regions: Vec<(usize, usize, String, Stats)> = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'r' && (i == 0 || !is_ident_byte(bytes[i - 1])) {
            let mut j = i + 1;
            while j < bytes.len() && bytes[j] == b'#' {
                j += 1;
            }
            if j < bytes.len() && bytes[j] == b'"' {
                let hashes = &text[i + 1..j];
                let closer = format!("\"{hashes}");
                if let Some(pos) = text[j + 1..].find(&closer) {
                    let lo = j + 1;
                    let hi = j + 1 + pos;
                    let content = &text[lo..hi];
                    if content.contains(".builder(") && content.contains("fn ") {
                        let plain_err = transform_source(content).err();
                        let attempt = match &plain_err {
                            None => transform_source(content),
                            Some(_) => {
                                let f = transform_format_template(content, true)
                                    .err()
                                    .map(|e| format!(" as format!: {}", e.0));
                                let r = transform_format_template(content, false)
                                    .err()
                                    .map(|e| format!(" as replace-token: {}", e.0));
                                match (f, r) {
                                    (None, _) | (_, None) => {
                                        transform_format_template(content, true)
                                            .or_else(|_| {
                                                transform_format_template(content, false)
                                            })
                                    }
                                    (Some(fe), Some(re)) => Err((
                                        format!("{pe}{fe}{re}", pe = plain_err.as_ref().unwrap().0),
                                        None,
                                    )),
                                }
                            }
                        };
                        if let Ok((new, stats)) = attempt {
                            regions.push((lo, hi, new, stats));
                        } else if let Err((e, _)) = attempt {
                            eprintln!("  (raw string at byte {lo} left untouched: {e})");
                        }
                    }
                    i = hi + closer.len();
                    continue;
                }
            }
        }
        i += 1;
    }
    let mut out = text.to_string();
    let mut stats = Stats::default();
    for (lo, hi, new, s) in regions.iter().rev() {
        out.replace_range(lo..hi, new);
        stats.folds += s.folds;
        stats.units += s.units;
        stats.removals += s.removals;
        stats.mut_kept += s.mut_kept;
    }
    Ok((out, stats))
}

fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

// ------------------------------------------------- format! templates

/// Transform a templated fixture. Two template dialects:
///   * `format_dialect` — `format!(..)` literals: doubled braces are
///     literal, every other `{..}` is a hole; the output re-encodes.
///   * otherwise — `.replace("{TOKEN}")` style: only ALL-CAPS tokens are
///     holes; every other brace is real rut and survives verbatim.
///
/// Holes ride through as sentinel idents; a hole the parser rejects in
/// expression position flips adaptively to a statement-form sentinel.
fn transform_format_template(
    content: &str,
    format_dialect: bool,
) -> Result<(String, Stats), (String, Option<usize>)> {
    const P: &str = "ZqHole0";
    if !content.contains('{') {
        return Err(("no braces".to_string(), None));
    }
    let pre = if format_dialect {
        content.replace("{{", "\x01").replace("}}", "\x02")
    } else {
        content.to_string()
    };
    if pre.to_lowercase().contains("zqhole0") {
        return Err(("sentinel collision".to_string(), None));
    }

    // scan brace pairs once: hole spans (byte ranges in `pre`) + texts
    let mut holes: Vec<(usize, usize, String)> = Vec::new(); // (start, end, text)
    {
        let mut rest = pre.as_str();
        let mut base = pre.len() - rest.len();
        while let Some(open) = rest.find('{') {
            let Some(close_rel) = rest[open..].find('}') else {
                return Err(("unclosed hole".to_string(), None));
            };
            let close = close_rel + open;
            let body = &rest[open + 1..close];
            let is_hole = if format_dialect {
                true
            } else {
                !body.is_empty()
                    && body
                        .chars()
                        .all(|c| c.is_ascii_uppercase() || c == '_' || c.is_ascii_digit())
            };
            if is_hole {
                holes.push((base + open, base + close + 1, rest[open..=close].to_string()));
                rest = &rest[close + 1..];
            } else {
                // not a hole — advance past the opening brace only (the
                // matching `}` may belong to real rut structure)
                rest = &rest[open + 1..];
            }
            base = pre.len() - rest.len();
        }
    }
    if std::env::var("RUT_FLUENT_DEBUG").is_ok() {
        eprintln!("  [template dialect={format_dialect} holes={}", holes.len());
    }
    if holes.is_empty() {
        return Err(("no holes".to_string(), None));
    }
    if holes.len() >= 100 {
        return Err(("too many holes".to_string(), None));
    }

    let build = |forms: &[String]| -> String {
        let mut out = String::with_capacity(pre.len());
        let mut last = 0usize;
        for (k, (start, end, _)) in holes.iter().enumerate() {
            out.push_str(&pre[last..*start]);
            out.push_str(&forms[k]);
            last = *end;
        }
        out.push_str(&pre[last..]);
        out
    };

    // expression sentinels first; a hole the parser rejects in expression
    // position flips to a statement-form sentinel (`zqhole0k();`)
    let mut forms: Vec<String> = (0..holes.len()).map(|k| format!("{P}{k:02}")).collect();
    let (mut out, stats) = loop {
        let cur = build(&forms).replace('\x01', "{").replace('\x02', "}");
        match transform_source(&cur) {
            Ok(pair) => break pair,
            Err((msg, at)) => {
                let Some(at) = at else {
                    return Err((msg, None));
                };
                // which sentinel span covers the failing byte? The parser
                // names the OFFENDING FOLLOWING token (e.g. the `let` after
                // a statement-position sentinel), so fall back to the
                // nearest preceding sentinel.
                let full = build(&forms);
                let mut search_from = 0usize;
                let mut flipped = false;
                let mut last_preceding: Option<usize> = None;
                for (k, f) in forms.iter().enumerate() {
                    let start = full[search_from..].find(f).expect("sentinel present") + search_from;
                    let end = start + f.len();
                    search_from = end;
                    if at >= start && at < end {
                        forms[k] = format!("zqhole0{k:02}();");
                        flipped = true;
                        break;
                    }
                    if end <= at {
                        last_preceding = Some(k);
                    }
                }
                if !flipped {
                    match last_preceding {
                        Some(k) => forms[k] = format!("zqhole0{k:02}();"),
                        None => return Err((msg, None)),
                    }
                }
            }
        }
    };
    if format_dialect {
        // every brace in the output is a template literal — re-encode it
        out = out.replace('{', "{{").replace('}', "}}");
    }
    for (idx, (_, _, hole)) in holes.iter().enumerate().rev() {
        out = out.replace(&forms[idx], hole);
    }
    Ok((out, stats))
}
// ------------------------------------------------------ source transform

fn transform_source(src: &str) -> Result<(String, Stats), (String, Option<usize>)> {
    let (toks, tdiags) = rut_lexer::lexer::lex_mode(src);
    if !tdiags.is_empty() {
        return Err((format!("lex diagnostics: {}", tdiags[0].msg), None));
    }
    let (ast, diags) = rut_parser::parse(src, Mode::Impl);
    if !diags.is_empty() {
        let at = diags[0].span.lo as usize;
        let ctx: String = src[at..].chars().take(40).collect();
        return Err((
            format!("parse diagnostics at byte {at} ({}): ...{}", diags[0].msg, ctx),
            Some(at),
        ));
    }
    let mut cx = Cx {
        src,
        ast: &ast,
        toks: &toks,
        edits: Vec::new(),
        covered: Vec::new(),
        stats: Stats::default(),
    };

    let all = collect_with_parent(&ast, ast.root.into(), None);
    // per-fn: plan the folds (mut decisions scoped to the enclosing fn)
    for (id, _) in &all {
        let body = match cx.ast.kind(*id) {
            Kind::Item(ItemKind::Fn(f)) => f.body.id(),
            Kind::Member(MemberKind::MethodDecl(m)) => match &m.body {
                Some(b) => b.id(),
                None => continue,
            },
            _ => continue,
        };
        let fn_nodes = collect_with_parent(&ast, body, Some(*id));
        plan_fns_blocks(&mut cx, &fn_nodes);
    }

    // global `.builder` removal outside folds
    remove_dot_builder(&mut cx, &all);

    let mut edits = std::mem::take(&mut cx.edits);
    edits.sort_by(|a, b| b.lo.cmp(&a.lo).then(b.hi.cmp(&a.hi)));
    for w in edits.windows(2) {
        if w[0].lo < w[1].hi {
            return Err((
                format!(
                    "overlapping edits at {}..{} and {}..{}",
                    w[1].lo, w[1].hi, w[0].lo, w[0].hi
                ),
                None,
            ));
        }
    }
    let mut out = src.to_string();
    for e in &edits {
        out.replace_range(e.lo..e.hi, &e.text);
        if std::env::var("RUT_FLUENT_DEBUG").is_ok() {
            eprintln!("edit {}..{} -> {:?}", e.lo, e.hi, e.text);
        }
    }

    // verify: the rewritten source must parse clean
    let (_, diags2) = rut_parser::parse(&out, Mode::Impl);
    if !diags2.is_empty() {
        if std::env::var("RUT_FLUENT_DEBUG").is_ok() {
            eprintln!("---- output ----\n{out}\n---- end ----");
        }
        return Err((
            format!("rewritten source failed to parse: {}", diags2[0].msg),
            None,
        ));
    }
    let leftover = out.matches(".builder(").count();
    if leftover > 0 {
        eprintln!("  warning: {leftover} `.builder(` occurrence(s) remain (comments?)");
    }
    Ok((out, cx.stats))
}

struct Cx<'a> {
    src: &'a str,
    ast: &'a Ast,
    toks: &'a [Token],
    edits: Vec<Edit>,
    /// source ranges whose text is rebuilt wholesale (fold heads, unit
    /// deletions) — `.builder` calls inside them are handled by the
    /// rebuild itself, never by the global removal walk
    covered: Vec<(usize, usize)>,
    stats: Stats,
}

fn plan_fns_blocks(cx: &mut Cx, fn_nodes: &[(NodeId, Option<NodeId>)]) {
    for (id, _) in fn_nodes {
        if let Kind::Expr(ExprKind::Block { stmts }) = cx.ast.kind(*id) {
            let stmts: Vec<NodeId> = stmts.iter().map(|h| h.id()).collect();
            plan_block(cx, &stmts, fn_nodes);
        }
    }
}

fn plan_block(cx: &mut Cx, stmts: &[NodeId], fn_nodes: &[(NodeId, Option<NodeId>)]) {
    let mut i = 0;
    while i < stmts.len() {
        let consumed = plan_fold(cx, stmts, i, fn_nodes);
        i += consumed.max(1);
    }
}

/// Plan one fold starting at `stmts[start]`. Returns the number of
/// statements consumed (0 = no fold planned).
fn plan_fold(
    cx: &mut Cx,
    stmts: &[NodeId],
    start: usize,
    fn_nodes: &[(NodeId, Option<NodeId>)],
) -> usize {
    let StmtKind::LetStmt {
        is_mut,
        name: name_id,
        destructure: None,
        ty: None,
        init,
    } = stmt_of(cx.ast, stmts[start])
    else {
        return 0;
    };
    let name = cx.ast.name(*name_id).to_string();
    if builder_shape(cx, init.id()).is_none() {
        return 0;
    }

    // the run: contiguous `x.m(..)` statements, units inlined (rule 3),
    // stopped by any interleaved statement or comment (rules 1-2)
    let let_span = cx.ast.span(stmts[start]);
    let mut elems: Vec<Elem> = Vec::new();
    let mut prev_end = head_close_hi(cx, init.id());
    let mut j = start + 1;
    while j < stmts.len() {
        let sp = cx.ast.span(stmts[j]);
        if !gap_clean(cx, prev_end, sp.lo as usize) {
            break;
        }
        match stmt_of(cx.ast, stmts[j]) {
            StmtKind::ExprStmt(m) => {
                if recv_is(cx, m.id(), &name) {
                    let close = call_close_hi(cx, m.id());
                    elems.push(Elem::Stmt {
                        stmt_lo: sp.lo as usize,
                        recv_hi: token_at(cx, sp.lo as usize).1,
                        close_hi: close,
                    });
                    prev_end = close;
                    j += 1;
                } else {
                    break;
                }
            }
            StmtKind::LetStmt {
                name: y_id,
                destructure: None,
                ty: None,
                init: y_init,
                ..
            } => {
                let y = cx.ast.name(*y_id).to_string();
                match try_unit(cx, stmts, j, &name, &y, y_init.id(), fn_nodes) {
                    Some((cons_close, elem, next)) => {
                        prev_end = cons_close;
                        elems.push(elem);
                        j = next;
                    }
                    None => break,
                }
            }
            _ => break,
        }
    }

    // mut decision (rule 4): any remaining mutating use of `name` outside
    // this fold's span keeps `let mut`
    let head_close = head_close_hi(cx, init.id());
    let last_hi = elems
        .last()
        .map_or(head_close, |e| match e {
            Elem::Stmt { close_hi, .. } => *close_hi,
            Elem::Unit { cons_close_hi, .. } => *cons_close_hi,
        });
    let fold_span = Span::new(let_span.lo, last_hi as u32);
    let keep_mut = *is_mut && mutating_uses_remain(cx, &name, fold_span, fn_nodes);

    // text assembly + edits
    let head = head_text(cx, init.id());
    let flat_len = "let ".len()
        + if keep_mut { "mut ".len() } else { 0 }
        + name.len()
        + " = ".len()
        + head.len()
        + elems
            .iter()
            .map(|e| match e {
                Elem::Stmt { recv_hi, close_hi, .. } => close_hi - recv_hi,
                Elem::Unit { chain, .. } => chain.len() + ".build()".len(),
            })
            .sum::<usize>()
        + 1;
    let cont_indent = line_indent(cx, let_span.lo as usize) + 4;
    let break_lines = flat_len > 100;

    let mut edits = Vec::new();
    edits.push(Edit {
        lo: let_span.lo as usize,
        hi: head_close,
        text: format!(
            "let {}{} = {}",
            if keep_mut { "mut " } else { "" },
            name,
            head
        ),
    });
    let mut prev_end = head_close;
    for e in &elems {
        let next_lo = match e {
            Elem::Stmt { stmt_lo, .. } => *stmt_lo,
            Elem::Unit { unit_lo, .. } => *unit_lo,
        };
        edits.push(gap_edit(prev_end, next_lo, break_lines, cont_indent));
        match e {
            Elem::Stmt { stmt_lo, recv_hi, .. } => {
                edits.push(Edit {
                    lo: *stmt_lo,
                    hi: *recv_hi,
                    text: String::new(),
                });
                prev_end = match e {
                    Elem::Stmt { close_hi, .. } => *close_hi,
                    _ => unreachable!(),
                };
            }
            Elem::Unit {
                unit_lo,
                unit_hi,
                cons_lo,
                cons_hi_recv,
                cons_close_hi,
                build_lo,
                build_hi,
                chain,
            } => {
                edits.push(Edit {
                    lo: *unit_lo,
                    hi: *unit_hi,
                    text: String::new(),
                });
                edits.push(Edit {
                    lo: *cons_lo,
                    hi: *cons_hi_recv,
                    text: String::new(),
                });
                edits.push(Edit {
                    lo: *build_lo,
                    hi: *build_hi,
                    text: format!("{chain}.build()"),
                });
                prev_end = *cons_close_hi;
            }
        }
    }

    // covered: the head rebuild range + every unit deletion range
    cx.covered.push((let_span.lo as usize, head_close));
    for e in &elems {
        if let Elem::Unit { unit_lo, unit_hi, .. } = e {
            cx.covered.push((*unit_lo, *unit_hi));
        }
    }
    let _ = fold_span;
    cx.edits.extend(edits);
    cx.stats.folds += 1;
    if keep_mut {
        cx.stats.mut_kept += 1;
    }
    j - start
}

/// Try to form an inlinable unit at `stmts[at]` (a builder-let of `y`)
/// consumed by the immediately-following `x.m(y.build())` statement.
/// Returns (consumer close hi, Elem, next index).
fn try_unit(
    cx: &mut Cx,
    stmts: &[NodeId],
    at: usize,
    x: &str,
    y: &str,
    y_init_id: NodeId,
    fn_nodes: &[(NodeId, Option<NodeId>)],
) -> Option<(usize, Elem, usize)> {
    builder_shape(cx, y_init_id)?;
    // y's own contiguous statements
    let let_span = cx.ast.span(stmts[at]);
    let mut k = at + 1;
    let mut prev_end = head_close_hi(cx, y_init_id);
    while k < stmts.len() {
        let sp = cx.ast.span(stmts[k]);
        if !gap_clean(cx, prev_end, sp.lo as usize) {
            return None;
        }
        match stmt_of(cx.ast, stmts[k]) {
            StmtKind::ExprStmt(m) if recv_is(cx, m.id(), y) => {
                prev_end = call_close_hi(cx, m.id());
                k += 1;
            }
            _ => break,
        }
    }
    // consumer: x.m(y.build()) — a single whole-arg build call
    if k >= stmts.len() {
        return None;
    }
    let cons_span = cx.ast.span(stmts[k]);
    if !gap_clean(cx, prev_end, cons_span.lo as usize) {
        return None;
    }
    let StmtKind::ExprStmt(cons_m) = stmt_of(cx.ast, stmts[k]) else {
        return None;
    };
    let ExprKind::Method {
        recv: cons_recv,
        args,
        ..
    } = expr_of(cx.ast, cons_m.id())
    else {
        return None;
    };
    let ExprKind::Path { segs: cons_segs } = expr_of(cx.ast, cons_recv.id()) else {
        return None;
    };
    if cons_segs.len() != 1 || cx.ast.name(cons_segs[0].name) != x || args.len() != 1 {
        return None;
    }
    let ExprKind::Method {
        recv: build_recv,
        name: build_name,
        args: build_args,
        ..
    } = expr_of(cx.ast, args[0].id())
    else {
        return None;
    };
    if cx.ast.name(*build_name) != "build" || !build_args.is_empty() {
        return None;
    }
    let ExprKind::Path { segs: build_segs } = expr_of(cx.ast, build_recv.id()) else {
        return None;
    };
    if build_segs.len() != 1 || cx.ast.name(build_segs[0].name) != y {
        return None;
    }
    // single-use: y appears exactly unit_stmts + 1 times in the fn
    let uses = count_path_uses(cx, fn_nodes, y);
    if uses != (k - at - 1) + 1 {
        return None;
    }

    cx.stats.units += 1;
    // the unit's inline text: `Y(..).m1(..)...` (verbatim spans)
    let StmtKind::LetStmt { init: y_init, .. } = stmt_of(cx.ast, stmts[at]) else {
        unreachable!("unit anchor is a let");
    };
    let mut chain = head_text(cx, y_init.id());
    for &s in &stmts[at + 1..k] {
        if let StmtKind::ExprStmt(m) = stmt_of(cx.ast, s) {
            chain.push_str(&seg_text(cx, m.id()));
        }
    }

    let build_span = cx.ast.span(args[0].id());
    let build_hi = call_close_hi(cx, args[0].id());
    let cons_close = call_close_hi(cx, cons_m.id());
    let cons_hi_recv = token_at(cx, cons_span.lo as usize).1;
    Some((
        cons_close,
        Elem::Unit {
            unit_lo: let_span.lo as usize,
            unit_hi: cons_span.lo as usize,
            cons_lo: cons_span.lo as usize,
            cons_hi_recv,
            cons_close_hi: cons_close,
            build_lo: build_span.lo as usize,
            build_hi,
            chain,
        },
        k + 1,
    ))
}

// --------------------------------------------------------------- helpers

fn stmt_of(ast: &Ast, id: NodeId) -> &StmtKind {
    ast.stmt(NodeHandle::<AnyStmt>::new(id))
}

fn expr_of(ast: &Ast, id: NodeId) -> &ExprKind {
    ast.expr(NodeHandle::<AnyExpr>::new(id))
}

/// `expr_of` for generic walks — `None` when the node is not an expr.
fn as_expr(ast: &Ast, id: NodeId) -> Option<&ExprKind> {
    match ast.kind(id) {
        Kind::Expr(_) => Some(expr_of(ast, id)),
        _ => None,
    }
}

/// init must be `Path.builder(..)` — the foldable let initializer shape.
fn builder_shape(cx: &Cx, init_id: NodeId) -> Option<()> {
    let ExprKind::Method {
        recv,
        name,
        generics,
        ..
    } = expr_of(cx.ast, init_id)
    else {
        return None;
    };
    if !generics.is_empty() || cx.ast.name(*name) != "builder" {
        return None;
    }
    let ExprKind::Path { segs } = expr_of(cx.ast, recv.id()) else {
        return None;
    };
    if segs.len() != 1 || !segs[0].generics.is_empty() {
        return None;
    }
    Some(())
}

/// Is `e` a method call whose receiver is the single identifier `name`?
fn recv_is(cx: &Cx, e: NodeId, name: &str) -> bool {
    let ExprKind::Method { recv, .. } = expr_of(cx.ast, e) else {
        return false;
    };
    let ExprKind::Path { segs } = expr_of(cx.ast, recv.id()) else {
        return false;
    };
    segs.len() == 1 && cx.ast.name(segs[0].name) == name
}

fn count_path_uses(cx: &Cx, fn_nodes: &[(NodeId, Option<NodeId>)], name: &str) -> usize {
    fn_nodes
        .iter()
        .filter(|(id, _)| {
            matches!(as_expr(cx.ast, *id),
                Some(ExprKind::Path { segs }) if segs.len() == 1 && cx.ast.name(segs[0].name) == name)
        })
        .count()
}

/// Any remaining mutating use of `name` outside `fold_span`?
fn mutating_uses_remain(
    cx: &Cx,
    name: &str,
    fold_span: Span,
    fn_nodes: &[(NodeId, Option<NodeId>)],
) -> bool {
    for (id, parent) in fn_nodes {
        let sp = cx.ast.span(*id);
        if sp.lo >= fold_span.lo && sp.hi <= fold_span.hi {
            continue;
        }
        let Some(ExprKind::Path { segs }) = as_expr(cx.ast, *id) else {
            continue;
        };
        if segs.len() != 1 || cx.ast.name(segs[0].name) != name {
            continue;
        }
        let Some(p) = parent else { continue };
        let Kind::Expr(pe) = cx.ast.kind(*p) else {
            continue;
        };
        match pe {
            // receiver of a call: `build` is the kit's only non-mut method
            ExprKind::Method { recv, name: m, .. } if recv.id() == *id => {
                if cx.ast.name(*m) != "build" {
                    return true;
                }
            }
            // assignment target (or base of one)
            ExprKind::Assign { target, .. } if target.id() == *id => return true,
            ExprKind::Field { recv, .. } if recv.id() == *id => return true,
            ExprKind::Index { recv, .. } if recv.id() == *id => return true,
            _ => {}
        }
    }
    false
}

/// The gap between two consecutive source positions must hold no comment.
fn gap_clean(cx: &Cx, lo: usize, hi: usize) -> bool {
    !cx.src[lo..hi].contains('/')
}

/// `T.builder(args)` → `T(args)` text (args verbatim).
fn head_text(cx: &Cx, init_id: NodeId) -> String {
    let (i, _) = token_at(cx, cx.ast.span(init_id).lo as usize);
    assert_eq!(cx.toks[i + 1].tok, Tok::Dot);
    assert_eq!(cx.toks[i + 2].tok, Tok::Ident("builder".to_string()));
    assert_eq!(cx.toks[i + 3].tok, Tok::LParen);
    let recv_lo = cx.toks[i].span.lo as usize;
    let recv_hi = cx.toks[i].span.hi as usize;
    let close = matching_paren(cx, i + 3);
    let mut text = cx.src[recv_lo..recv_hi].to_string();
    if close > i + 4 {
        // args: first token after `(` .. last token before `)`
        let arg_lo = cx.toks[i + 4].span.lo as usize;
        let arg_hi = cx.toks[close - 1].span.hi as usize;
        text.push('(');
        text.push_str(&cx.src[arg_lo..arg_hi]);
        text.push(')');
    } else {
        text.push_str("()");
    }
    strip_dot_builder(&mut text);
    text
}

/// Verbatim slices can carry nested `.builder(` call sites (excluded from
/// the global removal walk because they sit inside a fold span) — strip
/// them textually.
fn strip_dot_builder(text: &mut String) {
    if text.contains(".builder(") {
        *text = text.replace(".builder(", "(");
    }
}

/// hi of the closing paren of the `T.builder(..)` call.
fn head_close_hi(cx: &Cx, init_id: NodeId) -> usize {
    let (i, _) = token_at(cx, cx.ast.span(init_id).lo as usize);
    assert_eq!(cx.toks[i + 3].tok, Tok::LParen);
    cx.toks[matching_paren(cx, i + 3)].span.hi as usize
}

/// `.name(args)` text of a method call (verbatim args).
fn seg_text(cx: &Cx, m: NodeId) -> String {
    let (i, _) = token_at(cx, cx.ast.span(m).lo as usize);
    assert_eq!(cx.toks[i + 1].tok, Tok::Dot);
    let dot_lo = cx.toks[i + 1].span.lo as usize;
    let close_hi = call_close_hi(cx, m);
    let mut text = cx.src[dot_lo..close_hi].to_string();
    strip_dot_builder(&mut text);
    text
}

fn gap_edit(lo: usize, hi: usize, break_lines: bool, cont_indent: usize) -> Edit {
    let text = if break_lines {
        format!("\n{}", " ".repeat(cont_indent))
    } else {
        String::new()
    };
    Edit { lo, hi, text }
}

fn call_close_hi(cx: &Cx, m: NodeId) -> usize {
    let (i, _) = token_at(cx, cx.ast.span(m).lo as usize);
    // walk to the call's `(` (the receiver here is a single identifier)
    let mut k = i;
    while cx.toks[k].tok != Tok::LParen {
        k += 1;
    }
    cx.toks[matching_paren(cx, k)].span.hi as usize
}

fn matching_paren(cx: &Cx, open: usize) -> usize {
    let mut depth = 0i32;
    for (k, t) in cx.toks.iter().enumerate().skip(open) {
        match t.tok {
            Tok::LParen => depth += 1,
            Tok::RParen => {
                depth -= 1;
                if depth == 0 {
                    return k;
                }
            }
            _ => {}
        }
    }
    panic!("unbalanced parens at token {open}");
}

/// (token index, hi) of the token starting exactly at byte `lo`.
fn token_at(cx: &Cx, lo: usize) -> (usize, usize) {
    let idx = cx
        .toks
        .binary_search_by(|t| t.span.lo.cmp(&(lo as u32)))
        .unwrap_or_else(|_| panic!("no token starts at byte {lo}"));
    (idx, cx.toks[idx].span.hi as usize)
}

/// indentation (leading spaces) of the line containing byte `lo`
fn line_indent(cx: &Cx, lo: usize) -> usize {
    let line_start = cx.src[..lo].rfind('\n').map_or(0, |p| p + 1);
    let rest = &cx.src[line_start..];
    rest.len() - rest.trim_start_matches(' ').len()
}

// --------------------------------------------------- `.builder` removals

fn remove_dot_builder(cx: &mut Cx, all: &[(NodeId, Option<NodeId>)]) {
    for (id, _) in all {
        let Some(ExprKind::Method {
            recv,
            name,
            generics,
            ..
        }) = as_expr(cx.ast, *id)
        else {
            continue;
        };
        if !generics.is_empty() || cx.ast.name(*name) != "builder" {
            continue;
        }
        let sp = cx.ast.span(*id);
        if cx.covered.iter().any(|&(lo, hi)| sp.lo as usize >= lo && (sp.lo as usize) < hi) {
            continue;
        }
        let ExprKind::Path { segs } = expr_of(cx.ast, recv.id()) else {
            eprintln!("  warning: `.builder` on a complex receiver left untouched");
            continue;
        };
        if segs.len() != 1 {
            eprintln!("  warning: `.builder` on a multi-segment path left untouched");
            continue;
        }
        let (i, _) = token_at(cx, sp.lo as usize);
        assert_eq!(cx.toks[i + 1].tok, Tok::Dot);
        assert_eq!(cx.toks[i + 2].tok, Tok::Ident("builder".to_string()));
        cx.edits.push(Edit {
            lo: cx.toks[i + 1].span.lo as usize,
            hi: cx.toks[i + 2].span.hi as usize,
            text: String::new(),
        });
        cx.stats.removals += 1;
    }
}

// ---------------------------------------------------------- AST walking

/// Every (node, parent) pair in the subtree.
fn collect_with_parent(
    ast: &Ast,
    root: NodeId,
    parent: Option<NodeId>,
) -> Vec<(NodeId, Option<NodeId>)> {
    let mut out = Vec::new();
    walk(ast, root, parent, &mut out);
    out
}

fn walk(ast: &Ast, id: NodeId, parent: Option<NodeId>, out: &mut Vec<(NodeId, Option<NodeId>)>) {
    out.push((id, parent));
    let mut kids: Vec<NodeId> = Vec::new();
    match ast.kind(id) {
        Kind::Item(k) => match k {
            ItemKind::Module { items } => kids.extend(items.iter().map(|h| h.id())),
            ItemKind::Fn(f) => kids.push(f.body.id()),
            ItemKind::ModuleLet { init, .. } => kids.push(init.id()),
            ItemKind::Struct { methods, .. }
            | ItemKind::Class { methods, .. }
            | ItemKind::Impl { methods, .. }
            | ItemKind::Interface { methods, .. } => {
                kids.extend(methods.iter().map(|h| h.id()));
            }
            _ => {}
        },
        Kind::Member(k) => match k {
            MemberKind::MethodDecl(m) => {
                if let Some(b) = &m.body {
                    kids.push(b.id());
                }
                kids.extend(m.params.iter().map(|h| h.id()));
            }
            MemberKind::FieldDecl(f) => {
                kids.push(f.ty.id());
                if let Some(i) = &f.init {
                    kids.push(i.id());
                }
            }
            MemberKind::Param(p) => {
                if let Some(t) = &p.ty {
                    kids.push(t.id());
                }
            }
            _ => {}
        },
        Kind::Stmt(k) => match k {
            StmtKind::LetStmt { init, .. } => kids.push(init.id()),
            StmtKind::If { cond, then, els } => {
                kids.push(cond.id());
                kids.push(then.id());
                if let Some(e) = els {
                    match e {
                        ElseBranch::If(h) => kids.push(h.id()),
                        ElseBranch::Block(b) => kids.push(b.id()),
                    }
                }
            }
            StmtKind::While { cond, body } => {
                kids.push(cond.id());
                kids.push(body.id());
            }
            StmtKind::ForOf { iter, body, .. } => {
                kids.push(iter.id());
                kids.push(body.id());
            }
            StmtKind::ForC {
                init,
                cond,
                update,
                body,
                ..
            } => {
                kids.push(init.id());
                kids.push(cond.id());
                kids.push(update.id());
                kids.push(body.id());
            }
            StmtKind::Return { value } => {
                if let Some(v) = value {
                    kids.push(v.id());
                }
            }
            StmtKind::WhenStmt { scrut, arms } => {
                kids.push(scrut.id());
                kids.extend(arms.iter().map(|h| h.id()));
            }
            StmtKind::ExprStmt(e) => kids.push(e.id()),
            StmtKind::Break | StmtKind::Continue => {}
        },
        Kind::Expr(k) => match k {
            ExprKind::Block { stmts } => kids.extend(stmts.iter().map(|h| h.id())),
            ExprKind::Call { callee, args } => {
                kids.push(callee.id());
                kids.extend(args.iter().map(|h| h.id()));
            }
            ExprKind::Method { recv, args, .. } => {
                kids.push(recv.id());
                kids.extend(args.iter().map(|h| h.id()));
            }
            ExprKind::Field { recv, .. } | ExprKind::Index { recv, .. } => kids.push(recv.id()),
            ExprKind::Unary { expr, .. } | ExprKind::Try { expr } => kids.push(expr.id()),
            ExprKind::Binary { lhs, rhs, .. } => {
                kids.push(lhs.id());
                kids.push(rhs.id());
            }
            ExprKind::Assign { target, value, .. } => {
                kids.push(target.id());
                kids.push(value.id());
            }
            ExprKind::Lambda { params, body, .. } => {
                kids.extend(params.iter().map(|h| h.id()));
                kids.push(body.id());
            }
            ExprKind::FStr { parts } => {
                for p in parts {
                    if let FPartAst::Hole(h) = p {
                        kids.push(h.id());
                    }
                }
            }
            ExprKind::Struct { fields, .. } => {
                kids.extend(fields.iter().map(|(_, e)| e.id()));
            }
            ExprKind::Tuple { elems } | ExprKind::ArrayLit { elems } => {
                kids.extend(elems.iter().map(|h| h.id()));
            }
            ExprKind::ArrayRepeat { value, count } => {
                kids.push(value.id());
                kids.push(count.id());
            }
            ExprKind::WhenExpr { scrut, arms } => {
                kids.push(scrut.id());
                kids.extend(arms.iter().map(|h| h.id()));
            }
            ExprKind::Await { expr } => kids.push(expr.id()),
            ExprKind::AsyncBlock { body } => kids.push(body.id()),
            ExprKind::Is { expr, .. } | ExprKind::Cast { expr, .. } => kids.push(expr.id()),
            ExprKind::Lit(_) | ExprKind::Path { .. } => {}
        },
        Kind::Arm(ArmKind::WhenArm { pats, body }) => {
            for p in pats {
                if let Kind::Pat(PatKind::PatLit(e)) = ast.kind(p.id()) {
                    kids.push(e.id());
                }
            }
            kids.push(body.id());
        }
        Kind::Pat(_) | Kind::Type(_) => {}
    }
    for kid in kids {
        walk(ast, kid, Some(id), out);
    }
}
