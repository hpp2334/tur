// Audit: report legacy props-object constructor calls still present in Rust
// test fixtures (raw-string JS). Usage:
//   node scripts/audit-builder-legacy.mjs <rs files...>
import fs from "node:fs";
import path from "node:path";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const req = createRequire(path.resolve(here, "..", "js", "package.json"));
const ts = req("typescript");

const NAMES = new Set([
    "Container","SizedBox","Column","Row","Expanded","Stack","Positioned","Grid","Table","Text","Input","Image","ScrollView","Scrollbar","LazyList","LazyGrid","Opacity","Transform","CompositedTransformTarget","CompositedTransformFollower","Condition","Switch","Each","Fragment","PointerInteract","MouseRegion","Focusable","VirtualAppView","ReadableSubscribe","AnimatedContainer","AnimatedOpacity","AnimatedPositioned",
]);

function audit(file) {
    const text = fs.readFileSync(file, "utf8");
    let i = 0;
    const out = [];
    while (i < text.length) {
        if (text[i] === "r") {
            let j = i + 1;
            let h = 0;
            while (text[j] === "#") { h++; j++; }
            if (text[j] === '"') {
                const start = j + 1;
                const term = '"' + "#".repeat(h);
                const end = text.indexOf(term, start);
                if (end !== -1) {
                    let content = text.slice(start, end);
                    const ph = [];
                    content = content.replace(/\{([a-zA-Z_]\w*)\}/g, (m) => (ph.push(m), `__turFmtArg${ph.length - 1}__`));
                    content = content.replace(/\{\{/g, "{").replace(/\}\}/g, "}");
                    const sf = ts.createSourceFile("f", content, ts.ScriptTarget.Latest, true, ts.ScriptKind.TS);
                    if (sf.parseDiagnostics.length === 0) {
                        const walk = (n) => {
                            if (
                                ts.isCallExpression(n) &&
                                ts.isIdentifier(n.expression) &&
                                NAMES.has(n.expression.text) &&
                                n.arguments.length === 1 &&
                                ts.isObjectLiteralExpression(n.arguments[0]) &&
                                !ts.isPropertyAccessExpression(n.parent)
                            ) {
                                out.push(`  ${file} ${n.expression.text}(...)  line ${sf.getLineAndCharacterOfPosition(n.getStart(sf)).line + 1}`);
                            }
                            ts.forEachChild(n, walk);
                        };
                        walk(sf);
                    }
                    i = end + term.length;
                    continue;
                }
            }
        }
        i++;
    }
    return out;
}

let total = 0;
for (const f of process.argv.slice(2)) {
    const hits = audit(f);
    total += hits.length;
    for (const h of hits) console.log(h);
}
console.log("total legacy ctor calls:", total);
