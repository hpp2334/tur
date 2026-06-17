// One-shot: convert each edgy-case from `render(() => X);` to
// `export default component(() => X);`. Run once; idempotent (skips files
// that already `export default`).
const fs = require("node:fs");
const path = require("node:path");

const casesDir = path.join(__dirname, "..", "js/packages/tur-test-cases/edgy-cases");
let converted = 0;
let skipped = 0;

for (const dir of fs.readdirSync(casesDir)) {
    const file = path.join(casesDir, dir, "index.ts");
    if (!fs.existsSync(file)) continue;
    let src = fs.readFileSync(file, "utf8");

    if (/^\s*export\s+default\b/m.test(src)) {
        skipped++;
        continue;
    }
    if (!/\brender\(/.test(src)) {
        console.warn(`! ${dir}: no render() call, skipping`);
        skipped++;
        continue;
    }

    // 1) Replace the top-level `render(` call (always at line start in these
    //    cases) with `export default component(`.
    const callRe = /^(\s*)render\(/m;
    if (!callRe.test(src)) {
        console.warn(`! ${dir}: render( not at line start, skipping`);
        skipped++;
        continue;
    }
    src = src.replace(callRe, "$1export default component(");

    // 2) The only remaining `render` token is in the import — swap it for
    //    `component`.
    src = src.replace(/\brender\b/g, "component");

    fs.writeFileSync(file, src);
    converted++;
}

console.log(`converted=${converted} skipped=${skipped}`);
