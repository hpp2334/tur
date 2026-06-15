import fs from "node:fs";
import https from "node:https";
import path from "node:path";
import { fileURLToPath } from "node:url";
import type { CDPSession } from "playwright";
import { chromium, type Page } from "playwright";
import { generate as generateCert } from "selfsigned";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const DIST_DIR = path.resolve(__dirname, "../../tur-demo/dist");
const RESULTS_DIR = path.resolve(__dirname, "../test-results");
const PORT = 4030;

const MIME_TYPES: Record<string, string> = {
    ".html": "text/html",
    ".js": "text/javascript",
    ".mjs": "text/javascript",
    ".css": "text/css",
    ".wasm": "application/wasm",
    ".bin": "text/plain",
    ".map": "application/json",
    ".d.ts": "text/typescript",
    ".png": "image/png",
    ".svg": "image/svg+xml",
    ".json": "application/json",
    ".ttf": "font/ttf",
    ".woff": "font/woff",
    ".woff2": "font/woff2",
};

async function createServer(): Promise<https.Server> {
    const attrs = [{ name: "commonName", value: "localhost" }];
    const { cert, private: key } = await generateCert(attrs, {
        days: 365,
        algorithm: "sha256" as const,
        extensions: [
            {
                name: "subjectAltName",
                altNames: [{ type: 2, value: "localhost" }],
            },
        ],
    });
    return new Promise((resolve, reject) => {
        const server = https.createServer({ cert, key }, (req, res) => {
            const urlPath =
                req.url === "/" ? "/index.html" : (req.url as string);
            const filePath = path.join(DIST_DIR, urlPath);
            if (!filePath.startsWith(DIST_DIR)) {
                res.writeHead(403);
                res.end("Forbidden");
                return;
            }
            if (!fs.existsSync(filePath)) {
                res.writeHead(404);
                res.end("Not Found");
                return;
            }
            const ext = path.extname(filePath);
            const content = fs.readFileSync(filePath);
            res.writeHead(200, {
                "Content-Type": MIME_TYPES[ext] || "application/octet-stream",
                "Content-Length": content.length,
                "Cross-Origin-Opener-Policy": "same-origin",
                "Cross-Origin-Embedder-Policy": "require-corp",
            });
            res.end(content);
        });
        server.listen(PORT, () => resolve(server));
        server.on("error", reject);
    });
}

interface LayoutElement {
    type: string;
    label: string;
    x: number;
    y: number;
    w: number;
    h: number;
}

function parseLayout(layout: string): LayoutElement[] {
    const elements: LayoutElement[] = [];
    for (const line of layout.split("\n")) {
        const m = line.match(/abs\(([^,]+),([^)]+)\)\s+([\d.]+)x([\d.]+)/);
        if (!m) continue;
        const typeMatch = line.match(/\b(tur_\S+)/);
        const labelMatch = line.match(/"([^"]*)"/);
        elements.push({
            type: typeMatch?.[1] ?? "unknown",
            label: labelMatch?.[1] ?? "",
            x: parseFloat(m[1]),
            y: parseFloat(m[2]),
            w: parseFloat(m[3]),
            h: parseFloat(m[4]),
        });
    }
    return elements;
}

async function getElements(page: Page): Promise<LayoutElement[]> {
    const layout = (await page.evaluate(`
        (function() {
            var w = window;
            if (!w.turDemo) return "";
            return w.turDemo.debugLayout();
        })()
    `)) as string;
    return parseLayout(layout || "");
}

function countByLabel(elements: LayoutElement[], prefix: string): number {
    return elements.filter((e) => e.label.startsWith(prefix)).length;
}

async function selectCase(page: Page, caseName: string) {
    const btn = page.locator("button.case-item", {
        hasText: new RegExp(`^${caseName}$`),
    });
    await btn.click();
    await page.waitForFunction(
        (_name) => {
            const w = window as Record<string, unknown>;
            if (!w.turDemo) return false;
            try {
                const layout = (
                    w.turDemo as { debugLayout: () => string }
                ).debugLayout();
                return typeof layout === "string" && layout.length > 100;
            } catch {
                return false;
            }
        },
        caseName,
        { timeout: 30000 },
    );
}

async function getPerformanceMetrics(
    cdp: CDPSession,
): Promise<Record<string, number>> {
    const result = await cdp.send("Performance.getMetrics");
    const map: Record<string, number> = {};
    for (const m of result.metrics) {
        map[m.name] = m.value;
    }
    return map;
}

function fmt(ms: number): string {
    return `${ms.toFixed(1)}ms`;
}

function fmtMB(bytes: number): string {
    return `${(bytes / 1024 / 1024).toFixed(1)}MB`;
}

// Dispatch N scroll events in rapid succession and measure with performance.now()
async function scrollAndMeasure(
    page: Page,
    steps: number,
    deltaX: number,
    deltaY: number,
): Promise<{
    totalTimeMs: number;
    perStepMs: number[];
    elementsBefore: number;
    elementsAfter: number;
    labelsBefore: string;
    labelsAfter: string;
}> {
    const before = await getElements(page);
    const labelsBefore = before
        .map((e) => `${e.type}:"${e.label}"`)
        .sort()
        .join("|");

    const result = await page.evaluate(
        async ({ steps, deltaX, deltaY }) => {
            const canvas = document.querySelector(
                "#tur-container canvas",
            ) as HTMLCanvasElement | null;
            if (!canvas) return { times: [], total: 0 };

            const rect = canvas.getBoundingClientRect();
            const cx = rect.x + rect.width / 2;
            const cy = rect.y + rect.height / 2;
            const times: number[] = [];

            const totalStart = performance.now();

            for (let i = 0; i < steps; i++) {
                const t0 = performance.now();
                canvas.dispatchEvent(
                    new WheelEvent("wheel", {
                        deltaX,
                        deltaY,
                        clientX: cx,
                        clientY: cy,
                        bubbles: true,
                        cancelable: true,
                    }),
                );
                // Let the engine process the event synchronously
                // (WASM bridge processes in same task)
                const t1 = performance.now();
                times.push(t1 - t0);

                // Yield to let requestAnimationFrame / setTimeout callbacks fire
                await new Promise<void>((r) =>
                    requestAnimationFrame(() => r()),
                );
                await new Promise<void>((r) => setTimeout(r, 0));
            }

            return { times, total: performance.now() - totalStart };
        },
        { steps, deltaX, deltaY },
    );

    const after = await getElements(page);
    const labelsAfter = after
        .map((e) => `${e.type}:"${e.label}"`)
        .sort()
        .join("|");

    return {
        totalTimeMs: result.total,
        perStepMs: result.times,
        elementsBefore: before.length,
        elementsAfter: after.length,
        labelsBefore,
        labelsAfter,
    };
}

async function main() {
    if (!fs.existsSync(DIST_DIR)) {
        console.error(`dist not found: ${DIST_DIR}`);
        process.exit(1);
    }
    fs.mkdirSync(RESULTS_DIR, { recursive: true });

    console.log("Starting server...");
    const server = await createServer();

    try {
        const browser = await chromium.launch({
            headless: true,
            args: [
                "--enable-unsafe-webgpu",
                "--use-angle=metal",
                "--ignore-gpu-blocklist",
                "--disable-software-rasterizer",
            ],
        });
        const ctx = await browser.newContext({
            ignoreHTTPSErrors: true,
            viewport: { width: 1600, height: 900 },
        });
        const page = await ctx.newPage();
        const cdp = await ctx.newCDPSession(page);

        // Enable CDP Performance domain
        await cdp.send("Performance.enable", { timeDomain: "timeTicks" });

        await page.goto(`https://localhost:${PORT}/`, { waitUntil: "load" });
        await page.waitForTimeout(500);

        // ================================================================
        // LAZY LIST
        // ================================================================
        console.log("\n========== LAZY LIST ==========");

        const m0 = await getPerformanceMetrics(cdp);
        await selectCase(page, "lazy-list");
        await page.waitForTimeout(500);
        const m1 = await getPerformanceMetrics(cdp);

        const lazyElements = await getElements(page);
        const lazyItems = countByLabel(lazyElements, "Item #");

        console.log("\n--- Initial Render ---");
        console.log(
            `  Elements: ${lazyElements.length}, Items: #1..#${lazyItems}`,
        );
        console.log(
            `  JS Heap:  ${fmtMB(m1.JSHeapUsedSize)} used / ${fmtMB(m1.JSHeapTotalSize)} total`,
        );
        console.log(
            `  Heap delta: ${fmtMB(m1.JSHeapUsedSize - m0.JSHeapUsedSize)}`,
        );
        console.log(`  DOM nodes: ${m1.Nodes}`);
        console.log(`  Layout count: ${m1.LayoutCount}`);
        console.log(`  Recalc style count: ${m1.RecalcStyleCount}`);

        // V8 CPU Profile: 30 rapid vertical scrolls
        console.log("\n--- V8 CPU Profile: 30 vertical scrolls ---");

        await cdp.send("Profiler.enable");
        await cdp.send("Profiler.start");

        const lazyScroll = await scrollAndMeasure(page, 30, 0, 120);

        const { profile: lazyProfile } = await cdp.send("Profiler.stop");

        const lazyLabelsChanged =
            lazyScroll.labelsBefore !== lazyScroll.labelsAfter;

        console.log(`  Wall time: ${fmt(lazyScroll.totalTimeMs)}`);
        console.log(
            `  Per-step dispatch: avg=${fmt(lazyScroll.perStepMs.reduce((a, b) => a + b, 0) / lazyScroll.perStepMs.length)} max=${fmt(Math.max(...lazyScroll.perStepMs))}`,
        );
        console.log(
            `  Elements: ${lazyScroll.elementsBefore} → ${lazyScroll.elementsAfter}`,
        );
        console.log(`  Tree mutated: ${lazyLabelsChanged}`);

        // Analyze V8 profile - find hottest functions
        const lazyHotFns = analyzeProfile(lazyProfile);
        console.log(`  V8 profile nodes: ${lazyProfile.nodes.length}`);
        console.log(`  Top functions by self time:`);
        for (const fn of lazyHotFns.slice(0, 10)) {
            console.log(
                `    ${fmt(fn.selfMs)} self / ${fmt(fn.totalMs)} total  ${fn.name}`,
            );
        }

        const m2 = await getPerformanceMetrics(cdp);
        console.log(`\n  After scroll metrics:`);
        console.log(`  JS Heap: ${fmtMB(m2.JSHeapUsedSize)} used`);
        console.log(`  Layout count delta: ${m2.LayoutCount - m1.LayoutCount}`);
        console.log(
            `  Recalc style delta: ${m2.RecalcStyleCount - m1.RecalcStyleCount}`,
        );

        // ================================================================
        // SCROLL VIEW
        // ================================================================
        console.log("\n========== SCROLL VIEW (baseline) ==========");

        const m3 = await getPerformanceMetrics(cdp);
        await selectCase(page, "scroll-list");
        await page.waitForTimeout(500);
        const m4 = await getPerformanceMetrics(cdp);

        const scrollElements = await getElements(page);

        console.log("\n--- Initial Render ---");
        console.log(`  Elements: ${scrollElements.length}`);
        console.log(
            `  JS Heap: ${fmtMB(m4.JSHeapUsedSize)} used / ${fmtMB(m4.JSHeapTotalSize)} total`,
        );
        console.log(
            `  Heap delta: ${fmtMB(m4.JSHeapUsedSize - m3.JSHeapUsedSize)}`,
        );
        console.log(`  DOM nodes: ${m4.Nodes}`);

        // V8 CPU Profile: 30 rapid vertical scrolls
        console.log("\n--- V8 CPU Profile: 30 vertical scrolls ---");

        await cdp.send("Profiler.enable");
        await cdp.send("Profiler.start");

        const scrollScroll = await scrollAndMeasure(page, 30, 0, 120);

        const { profile: scrollProfile } = await cdp.send("Profiler.stop");

        console.log(`  Wall time: ${fmt(scrollScroll.totalTimeMs)}`);
        console.log(
            `  Per-step dispatch: avg=${fmt(scrollScroll.perStepMs.reduce((a, b) => a + b, 0) / scrollScroll.perStepMs.length)} max=${fmt(Math.max(...scrollScroll.perStepMs))}`,
        );
        console.log(
            `  Elements: ${scrollScroll.elementsBefore} → ${scrollScroll.elementsAfter}`,
        );
        console.log(
            `  Tree mutated: ${scrollScroll.labelsBefore !== scrollScroll.labelsAfter}`,
        );

        const scrollHotFns = analyzeProfile(scrollProfile);
        console.log(`  V8 profile nodes: ${scrollProfile.nodes.length}`);
        console.log(`  Top functions by self time:`);
        for (const fn of scrollHotFns.slice(0, 10)) {
            console.log(
                `    ${fmt(fn.selfMs)} self / ${fmt(fn.totalMs)} total  ${fn.name}`,
            );
        }

        const m5 = await getPerformanceMetrics(cdp);
        console.log(`\n  After scroll metrics:`);
        console.log(`  JS Heap: ${fmtMB(m5.JSHeapUsedSize)} used`);
        console.log(`  Layout count delta: ${m5.LayoutCount - m4.LayoutCount}`);
        console.log(
            `  Recalc style delta: ${m5.RecalcStyleCount - m4.RecalcStyleCount}`,
        );

        // ================================================================
        // COMPARISON
        // ================================================================
        console.log("\n========== COMPARISON ==========");
        console.log("\n  Heap usage:");
        console.log(`    LazyColumn init:  ${fmtMB(m1.JSHeapUsedSize)}`);
        console.log(`    ScrollView init:  ${fmtMB(m4.JSHeapUsedSize)}`);
        console.log(`    LazyColumn after scroll: ${fmtMB(m2.JSHeapUsedSize)}`);
        console.log(`    ScrollView after scroll: ${fmtMB(m5.JSHeapUsedSize)}`);

        console.log("\n  V8 profile self-time (top 5):");
        console.log("    LazyColumn:");
        for (const fn of lazyHotFns.slice(0, 5)) {
            console.log(`      ${fmt(fn.selfMs)} ${fn.name}`);
        }
        console.log("    ScrollView:");
        for (const fn of scrollHotFns.slice(0, 5)) {
            console.log(`      ${fmt(fn.selfMs)} ${fn.name}`);
        }

        // Find functions only in lazy list profile (not in scroll view)
        const scrollFnNames = new Set(scrollHotFns.map((f) => f.name));
        const lazyOnly = lazyHotFns.filter((f) => !scrollFnNames.has(f.name));
        if (lazyOnly.length > 0) {
            console.log("\n  Functions only in LazyColumn profile:");
            for (const fn of lazyOnly.slice(0, 10)) {
                console.log(
                    `    ${fmt(fn.selfMs)} self / ${fmt(fn.totalMs)} total  ${fn.name}`,
                );
            }
        }

        // Write V8 profiles as JSON for further analysis
        const profileDir = path.join(RESULTS_DIR, "profiles");
        fs.mkdirSync(profileDir, { recursive: true });
        fs.writeFileSync(
            path.join(profileDir, "lazy-scroll.cpuprofile"),
            JSON.stringify(lazyProfile),
        );
        fs.writeFileSync(
            path.join(profileDir, "scroll-scroll.cpuprofile"),
            JSON.stringify(scrollProfile),
        );
        console.log(`\n  Profiles saved to ${profileDir}/`);
        console.log("  Open in Chrome DevTools → Performance tab to inspect");

        await browser.close();
    } finally {
        server.close();
    }
}

interface HotFunction {
    name: string;
    selfMs: number;
    totalMs: number;
}

function analyzeProfile(profile: {
    nodes: Array<{
        id: number;
        callFrame: { functionName: string; url: string };
        hitCount?: number;
        children?: number[];
    }>;
    samples?: number[];
    timeDeltas?: number[];
}): HotFunction[] {
    const nodesById = new Map<number, (typeof profile.nodes)[number]>();
    for (const node of profile.nodes) {
        nodesById.set(node.id, node);
    }

    // Accumulate self time from samples
    const selfTime = new Map<number, number>();

    if (profile.samples && profile.timeDeltas) {
        for (let i = 0; i < profile.samples.length; i++) {
            const nodeId = profile.samples[i];
            const delta = profile.timeDeltas[i] || 0;
            selfTime.set(nodeId, (selfTime.get(nodeId) || 0) + delta);

            // Walk up the call stack to add total time
            // The sample represents the full call stack at that moment
            // We need to reconstruct the stack path
            // For simplicity, just count self time
        }
    }

    // Also use hitCount from nodes as fallback
    for (const node of profile.nodes) {
        if (node.hitCount && node.hitCount > 0) {
            selfTime.set(node.id, (selfTime.get(node.id) || 0) + node.hitCount);
        }
    }

    const fns: HotFunction[] = [];
    for (const node of profile.nodes) {
        const self = selfTime.get(node.id) || 0;
        if (self === 0) continue;
        const name = node.callFrame.functionName || "(anonymous)";
        const url = node.callFrame.url;
        const label =
            url && !url.startsWith("wasm")
                ? `${name} (${shortenUrl(url)})`
                : name;
        fns.push({ name: label, selfMs: self / 1000, totalMs: self / 1000 });
    }

    fns.sort((a, b) => b.selfMs - a.selfMs);
    return fns;
}

function shortenUrl(url: string): string {
    try {
        const u = new URL(url);
        const parts = u.pathname.split("/");
        return parts.length > 2
            ? `${parts[parts.length - 2]}/${parts[parts.length - 1]}`
            : u.pathname;
    } catch {
        return url.slice(0, 60);
    }
}

main().catch((err) => {
    console.error(err);
    process.exit(1);
});
