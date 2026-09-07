#!/usr/bin/env node
/**
 * migrate-builder-codemod — rewrite legacy props-object element constructors
 * into the builder pattern:
 *
 *   Container({ padding: 16, children: [Text({ text: "hi" })] })
 *     → Container()
 *           .padding(16)
 *           .children([Text({ text: "hi" }).build()])
 *           .build()
 *
 * Rules (mirroring the engine's `core/js_runtime/builder.rs`):
 *   - Required props stay in the constructor object; everything else chains
 *     (camelCase method per prop, source order preserved).
 *   - `children: [...]` becomes `.children([...])` (last, before `.build()`).
 *   - `child: x` becomes `.child(x)` (after the plain props).
 *   - Collision renames: Each `build` → `itemBuilder`, Table `build` →
 *     `rowBuilder`, `buildHeader` → `headerBuilder`.
 *   - Non-decomposable props objects (spreads, computed keys, getters) fall
 *     back to the whole object in the constructor + `.build()` — still valid.
 *
 * Modes:
 *   node scripts/migrate-builder-codemod.mjs <ts/tsx/js files...>
 *   node scripts/migrate-builder-codemod.mjs --rust <rs files...>
 *     (transforms the JS/TS content of Rust raw strings `r#"..."#`; spans
 *      that fail to parse as TS are left untouched)
 *
 * The transform is span-splice based: only matched CallExpression spans are
 * rewritten (nested matches are spliced into their parent's slice), so all
 * surrounding formatting and comments are preserved. Run `biome format`
 * afterwards to reflow the rewritten chains.
 */

import fs from "node:fs";
import { createRequire } from "node:module";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

// Resolve `typescript` from the js workspace (the script may run from the
// repo root, which has no node_modules).
const here = path.dirname(fileURLToPath(import.meta.url));
const require = createRequire(pathToFileURL(path.join(here, "..", "js", "package.json")));
const ts = require("typescript");

// ---------------------------------------------------------------------------
// Constructor table — name → { req: required-in-ctor keys, child: 'child' |
// 'children' | null (which generic child method the element exposes),
// renames: prop key → builder method name }
// ---------------------------------------------------------------------------

const CTORS = {
    Container: { req: [], child: null, methods: "width,height,padding,color,borderColor,borderWidth,borderRadius,borderPosition,clipBehavior,shadowColor,shadowBlur,shadowOffset,alignment,queryKey" },
    SizedBox: { req: [], child: null, methods: "width,height,padding,color,borderColor,borderWidth,borderRadius,borderPosition,clipBehavior,shadowColor,shadowBlur,shadowOffset,alignment,queryKey" },
    Column: { req: [], child: null, methods: "mainAlignment,crossAlignment,mainAxisSize,queryKey" },
    Row: { req: [], child: null, methods: "mainAlignment,crossAlignment,mainAxisSize,queryKey" },
    Expanded: { req: [], child: "child", methods: "flex,queryKey" },
    Stack: { req: [], child: null, methods: "fit,alignment,queryKey" },
    Positioned: { req: [], child: "child", methods: "left,top,right,bottom,width,height,queryKey" },
    Grid: { req: ["maxCrossAxisExtent"], child: null, methods: "maxCrossAxisExtent,childAspectRatio,mainAxisExtent,crossAxisSpacing,mainAxisSpacing,queryKey" },
    Table: {
        req: ["columns", "rows"],
        child: null,
        methods: "columns,rows,rowBuilder,headerBuilder,headerExtent,rowExtent,rowSpacing,stripeColor,dividerColor,dividerThickness,queryKey",
        renames: { build: "rowBuilder", buildHeader: "headerBuilder" },
    },
    Text: { req: ["text"], child: null, methods: "text,fontSize,fontWeight,color,spans,maxLines,overflow,selectable,onSelectionChange,queryKey" },
    Input: { req: [], child: null, methods: "width,height,controller,undoController,placeholder,color,placeholderColor,cursorColor,fontSize,fontFamily,fontWeight,multiline,obscureText,obscuringCharacter,onContextMenu,queryKey" },
    Image: { req: ["resourceId"], child: "child", methods: "resourceId,width,height,fit,queryKey" },
    ScrollView: { req: [], child: "child", methods: "axis,padding,color,controller,queryKey" },
    Scrollbar: { req: [], child: null, methods: "color,trackColor,thumbRadius,thickness,queryKey" },
    LazyList: { req: ["itemCount"], child: null, methods: "itemCount,builder,axis,overscan,itemExtent,queryKey" },
    LazyGrid: { req: ["itemCount", "maxCrossAxisExtent"], child: null, methods: "itemCount,maxCrossAxisExtent,builder,axis,overscan,childAspectRatio,mainAxisExtent,crossAxisSpacing,mainAxisSpacing,queryKey" },
    Opacity: { req: ["value"], child: "child", methods: "value,queryKey" },
    Transform: { req: [], child: "child", methods: "scale,scaleX,scaleY,rotate,translateX,translateY,alignment,queryKey" },
    CompositedTransformTarget: { req: ["link"], child: "child", methods: "link" },
    CompositedTransformFollower: { req: ["link"], child: "child", methods: "link,targetAnchor,followerAnchor,targetOffset,showWhenUnlinked" },
    Condition: { req: ["condition"], child: "child", methods: "condition,elseChild,queryKey" },
    Switch: { req: ["value"], child: null, methods: "value,cases,fallback,queryKey" },
    Each: { req: ["items"], child: null, methods: "items,itemBuilder,queryKey", renames: { build: "itemBuilder" } },
    Fragment: { req: [], child: null, methods: "queryKey" },
    PointerInteract: { req: [], child: "child", methods: "behavior,onClick,onPointerDown,onPointerMove,onPointerUp,onContextMenu,queryKey" },
    MouseRegion: { req: [], child: "child", methods: "behavior,cursor,onEnter,onExit,queryKey" },
    Focusable: { req: [], child: "child", methods: "onKeyDown,onKeyUp,onFocus,onBlur" },
    VirtualAppView: { req: ["app$"], child: null, methods: "app$,background,width,height,queryKey,fallback,errorView" },
    ReadableSubscribe: { req: [], child: "child", methods: "readables,onUpdate$" },
    // JS widgets from tur:animation
    AnimatedContainer: { req: [], child: null, methods: "width,height,padding,color,borderColor,borderWidth,borderRadius,shadowColor,shadowBlur,alignment,borderPosition,shadowOffset,queryKey,children,duration,curve,onEnd" },
    AnimatedOpacity: { req: [], child: "child", methods: "value,duration,curve,onEnd,child,queryKey" },
    AnimatedPositioned: { req: [], child: "child", methods: "left,top,right,bottom,width,height,child,duration,curve,onEnd" },
};

// Precompute the method-name sets (renames applied: the METHOD for prop `k`).
for (const info of Object.values(CTORS)) {
    const methodNames = info.methods
        .split(",")
        .map((k) => info.renames?.[k] ?? k);
    info.methodSet = new Set(methodNames);
}

// ---------------------------------------------------------------------------
// AST helpers
// ---------------------------------------------------------------------------

function ctorInfo(node) {
    if (
        ts.isCallExpression(node) &&
        ts.isIdentifier(node.expression) &&
        Object.hasOwn(CTORS, node.expression.text) &&
        node.arguments.length >= 1 &&
        !isAlreadyBuilt(node)
    ) {
        return CTORS[node.expression.text];
    }
    return null;
}

function propName(p) {
    if (ts.isPropertyAssignment(p) && ts.isIdentifier(p.name)) return p.name.text;
    if (ts.isPropertyAssignment(p) && ts.isStringLiteral(p.name)) return p.name.text;
    if (ts.isShorthandPropertyAssignment(p)) return p.name.text;
    return null;
}

function collectMatchedCalls(node, out) {
    if (ctorInfo(node)) out.push(node);
    ts.forEachChild(node, (c) => collectMatchedCalls(c, out));
}

/** Skip calls already chained on (`X({...}).foo()` — legacy constructor
 *  results were never method-chained, so any property-access parent means
 *  the call was already migrated). */
function isAlreadyBuilt(call) {
    return ts.isPropertyAccessExpression(call.parent);
}

// ---------------------------------------------------------------------------
// The transform
// ---------------------------------------------------------------------------

class Transformer {
    constructor(sourceText, sourceFile) {
        this.text = sourceText;
        this.sf = sourceFile;
        this.memo = new Map(); // call node → replacement text
    }

    slice(start, end) {
        return this.text.slice(start, end);
    }

    /** Original slice of `node` with every matched ctor call inside it
     *  spliced to its replacement text (recursive, original spans). */
    emit(node) {
        if (ctorInfo(node)) return this.replacement(node);
        const start = node.getStart(this.sf);
        const end = node.end;
        // Find matched calls fully inside [start, end).
        const inner = this.matched.filter(
            (c) => c.getStart(this.sf) >= start && c.end <= end && c !== node,
        );
        if (inner.length === 0) return this.slice(start, end);
        // Keep only outermost within this slice.
        const outermost = inner.filter(
            (c) => !inner.some((o) => o !== c && contains(o, c)),
        );
        let out = "";
        let pos = start;
        for (const c of [...outermost].sort((a, b) => a.getStart(this.sf) - b.getStart(this.sf))) {
            const s = c.getStart(this.sf);
            out += this.slice(pos, s);
            out += this.replacement(c);
            pos = c.end;
        }
        out += this.slice(pos, end);
        return out;
    }

    replacement(call) {
        if (this.memo.has(call)) return this.memo.get(call);
        // Compute with a provisional placeholder to guard against (invalid)
        // cyclic nesting.
        this.memo.set(call, "/*cycle*/");
        const text = this.transformCall(call);
        this.memo.set(call, text);
        return text;
    }

    transformCall(call) {
        const info = ctorInfo(call);
        const name = call.expression.text;
        const typeArgs = call.typeArguments
            ? this.slice(call.typeArguments.pos - 1, call.typeArguments.end) // includes <...>
            : "";
        const ctorHead = `${name}${typeArgs}`;
        const indent = this.indentOf(call.getStart(this.sf));
        const arg = call.arguments[0];

        // Fallback: a non-object argument (a props variable) — pass through +
        // `.build()` (the runtime constructor accepts any props object).
        if (!ts.isObjectLiteralExpression(arg)) {
            return `${ctorHead}(${this.emit(arg)}).build()`;
        }

        const kept = [];
        const chained = [];
        let childrenInit = null;
        let childInit = null;
        let decomposable = true;

        for (const p of arg.properties) {
            const key = propName(p);
            if (key === null) {
                decomposable = false;
                break;
            }
            if (ts.isShorthandPropertyAssignment(p)) {
                const method = info.renames?.[key] ?? key;
                if (info.methodSet.has(method) && !info.req.includes(key)) {
                    chained.push({ key, text: key });
                } else {
                    kept.push(p);
                }
                continue;
            }
            const method = info.renames?.[key] ?? key;
            const chainable = info.methodSet.has(method);
            if (info.req.includes(key)) {
                kept.push(p);
            } else if (key === "children" && info.child !== "child") {
                childrenInit = p.initializer;
            } else if (key === "child" && info.child === "child") {
                childInit = p.initializer;
            } else if (chainable) {
                chained.push({ key, init: p.initializer });
            } else {
                // No builder method for this prop (stale/unknown — the old
                // runtime ignored it too): keep it in the constructor object,
                // preserving the legacy behavior exactly.
                kept.push(p);
            }
        }

        if (!decomposable) {
            // Whole object stays in the constructor (valid — the runtime
            // accepts any props there); still transform nested calls inside.
            return `${ctorHead}(${this.emit(arg)}).build()`;
        }

        const keptText = kept.map((p) => this.emit(p)).join(", ");
        const ctorText = kept.length > 0 ? `${ctorHead}({ ${keptText} })` : `${ctorHead}()`;

        const chain = [];
        for (const c of chained) {
            const method = info.renames?.[c.key] ?? c.key;
            const value = c.text ?? this.emit(c.init);
            chain.push(`.${method}(${value})`);
        }
        if (childInit) chain.push(`.child(${this.emit(childInit)})`);
        if (childrenInit) chain.push(`.children(${this.emit(childrenInit)})`);
        chain.push(".build()");

        const single = ctorText + chain.join("");
        const column = call.getStart(this.sf) - this.sf.getLineAndCharacterOfPosition(call.getStart(this.sf)).character;
        if (single.length + column <= 100 && !single.includes("\n") && chain.length <= 2) {
            return single;
        }
        return ctorText + chain.map((c) => `\n${indent}    ${c}`).join("");
    }

    indentOf(pos) {
        const { character } = this.sf.getLineAndCharacterOfPosition(pos);
        const lineStart = pos - character;
        const prefix = this.text.slice(lineStart, pos);
        const match = /[ \t]*$/.exec(prefix);
        return match ? match[0] : "";
    }

    run() {
        this.matched = [];
        collectMatchedCalls(this.sf, this.matched);
        if (this.matched.length === 0) return { text: this.text, count: 0 };
        const outermost = this.matched.filter(
            (c) => !this.matched.some((o) => o !== c && contains(o, c)),
        );
        let out = "";
        let pos = 0;
        for (const c of [...outermost].sort((a, b) => a.getStart(this.sf) - b.getStart(this.sf))) {
            const s = c.getStart(this.sf);
            out += this.text.slice(pos, s);
            out += this.replacement(c);
            pos = c.end;
        }
        out += this.text.slice(pos);
        return { text: out, count: outermost.length };
    }
}

function contains(outer, inner) {
    return outer.getStart(undefined) <= inner.getStart(undefined) && inner.end <= outer.end;
}

function transformSource(text, kind) {
    const sf = ts.createSourceFile("f", text, ts.ScriptTarget.Latest, true, kind);
    // Reject parses with syntax errors (e.g. .rs raw-string spans that aren't JS).
    const diags = [];
    sf.parseDiagnostics.forEach((d) => diags.push(d));
    if (diags.length > 0) return null;
    return new Transformer(text, sf).run();
}

// ---------------------------------------------------------------------------
// .rs mode — transform the JS/TS content of Rust raw strings
// ---------------------------------------------------------------------------

function rustRawStringSpans(text) {
    const spans = [];
    let i = 0;
    while (i < text.length) {
        if (text[i] === "r") {
            let j = i + 1;
            let hashes = 0;
            while (text[j] === "#") {
                hashes++;
                j++;
            }
            if (text[j] === '"') {
                const contentStart = j + 1;
                const terminator = '"' + "#".repeat(hashes);
                const end = text.indexOf(terminator, contentStart);
                if (end !== -1) {
                    spans.push({ contentStart, contentEnd: end });
                    i = end + terminator.length;
                    continue;
                }
            }
        }
        i++;
    }
    return spans;
}

function transformRustFile(text) {
    const spans = rustRawStringSpans(text);
    let out = text;
    let count = 0;
    // Reverse order so earlier offsets stay stable.
    for (const { contentStart, contentEnd } of [...spans].reverse()) {
        const content = out.slice(contentStart, contentEnd);
        if (!/[A-Za-z_]\w*\s*\(\s*\{/.test(content)) continue;
        const kind = content.includes("</") ? ts.ScriptKind.TSX : ts.ScriptKind.TS;
        let res = transformSource(content, kind) ?? transformSource(content, ts.ScriptKind.TS);
        if (!res) {
            // `format!` fixtures escape literal braces (`{{` / `}}`) and
            // substitute single-brace `{name}` placeholders. Direct parse
            // fails on those; protect placeholders → unescape → transform →
            // re-escape → restore. Only applied when the result parses AND
            // something transforms (otherwise the original is untouched).
            const placeholders = [];
            let unescaped = content.replace(/\{([a-zA-Z_]\w*)\}/g, (m, name) => {
                placeholders.push(m);
                return `__turFmtArg${placeholders.length - 1}__`;
            });
            unescaped = unescaped.replace(/\{\{/g, "{").replace(/\}\}/g, "}");
            const r2 =
                transformSource(unescaped, kind) ?? transformSource(unescaped, ts.ScriptKind.TS);
            if (r2 && r2.count > 0 && r2.text !== unescaped) {
                let reescaped = r2.text.replace(/\{/g, "{{").replace(/\}/g, "}}");
                reescaped = reescaped.replace(/__turFmtArg(\d+)__/g, (m, i) =>
                    placeholders[Number(i)] ?? m,
                );
                out = out.slice(0, contentStart) + reescaped + out.slice(contentEnd);
                count += r2.count;
            }
            continue;
        }
        if (res.count === 0) continue;
        out = out.slice(0, contentStart) + res.text + out.slice(contentEnd);
        count += res.count;
    }
    return { text: out, count };
}

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

function main() {
    const args = process.argv.slice(2);
    const rustMode = args[0] === "--rust";
    const files = rustMode ? args.slice(1) : args;
    if (files.length === 0) {
        console.error("usage: migrate-builder-codemod.mjs [--rust] <files...>");
        process.exit(1);
    }
    let total = 0;
    let touched = 0;
    for (const file of files) {
        const text = fs.readFileSync(file, "utf8");
        const res = rustMode
            ? transformRustFile(text)
            : (() => {
                  const kind = file.endsWith(".tsx")
                      ? ts.ScriptKind.TSX
                      : file.endsWith(".js") || file.endsWith(".mjs")
                        ? ts.ScriptKind.JS
                        : ts.ScriptKind.TS;
                  const r = transformSource(text, kind);
                  if (!r) throw new Error(`parse failed: ${file}`);
                  return r;
              })();
        if (res.count > 0 && res.text !== text) {
            fs.writeFileSync(file, res.text);
            touched++;
        }
        total += res.count;
        console.log(`${res.count > 0 ? "✓" : "·"} ${file} (${res.count})`);
    }
    console.log(`\ntransformed ${total} call(s) across ${touched}/${files.length} file(s)`);
}

main();
