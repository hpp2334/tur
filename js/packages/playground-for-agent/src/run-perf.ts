import fs from "node:fs";
import https from "node:https";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { type CDPSession, chromium, type Page } from "playwright";
import { generate as generateCert } from "selfsigned";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const DIST_DIR = path.resolve(__dirname, "../../tur-react-demo/dist");
const RESULTS_DIR = path.resolve(__dirname, "../test-results");
const PORT = 4010;

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
    const options = {
        days: 365,
        algorithm: "sha256" as const,
        extensions: [
            {
                name: "subjectAltName",
                altNames: [{ type: 2, value: "localhost" }],
            },
        ],
    };
    const { cert, private: key } = await generateCert(attrs, options);

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
            const contentType = MIME_TYPES[ext] || "application/octet-stream";
            const content = fs.readFileSync(filePath);
            const headers: Record<string, string | number> = {
                "Content-Type": contentType,
                "Content-Length": content.length,
                "Cross-Origin-Opener-Policy": "same-origin",
                "Cross-Origin-Embedder-Policy": "require-corp",
            };
            res.writeHead(200, headers);
            res.end(content);
        });

        server.listen(PORT, () => resolve(server));
        server.on("error", reject);
    });
}

interface PerfResult {
    name: string;
    metrics: Record<string, number>;
}

async function getCDPMetrics(cdp: CDPSession): Promise<Record<string, number>> {
    const result = await cdp.send("Performance.getMetrics");
    const map: Record<string, number> = {};
    for (const m of result.metrics) {
        map[m.name] = m.value;
    }
    return map;
}

async function measureTime<T>(
    _label: string,
    fn: () => Promise<T>,
): Promise<{ result: T; durationMs: number }> {
    const start = performance.now();
    const result = await fn();
    const durationMs = performance.now() - start;
    return { result, durationMs };
}

async function screenshot(page: Page, label: string) {
    const filePath = path.join(RESULTS_DIR, `perf-${label}.png`);
    await page.screenshot({ path: filePath, fullPage: true });
    return filePath;
}

async function selectCase(page: Page, caseName: string) {
    const btn = page.locator("button.case-item", {
        hasText: new RegExp(`^${caseName}$`),
    });
    await btn.click();

    await page.waitForFunction(
        (_name: string) => {
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

async function countElements(page: Page): Promise<number> {
    const layout: string | undefined = await page.evaluate(() => {
        const w = window as Record<string, unknown>;
        if (!w.turDemo) return "";
        return (w.turDemo as { debugLayout: () => string }).debugLayout();
    });
    if (!layout) return 0;
    let count = 0;
    for (const line of layout.split("\n")) {
        if (line.includes("abs(")) count++;
    }
    return count;
}

async function scrollCanvas(
    page: Page,
    deltaX: number,
    deltaY: number,
    steps: number,
): Promise<number[]> {
    return page.evaluate(
        async ({ deltaX, deltaY, steps }) => {
            const canvas = document.querySelector("#tur-container canvas");
            if (!canvas) return [];

            const rect = canvas.getBoundingClientRect();
            const cx = rect.x + rect.width / 2;
            const cy = rect.y + rect.height / 2;

            const frameTimes: number[] = [];

            for (let i = 0; i < steps; i++) {
                const t0 = performance.now();

                const event = new WheelEvent("wheel", {
                    deltaX,
                    deltaY,
                    clientX: cx,
                    clientY: cy,
                    bubbles: true,
                    cancelable: true,
                });
                canvas.dispatchEvent(event);

                await new Promise<void>((r) => {
                    requestAnimationFrame(() => r());
                });
                await new Promise<void>((r) => {
                    setTimeout(r, 16);
                });

                const t1 = performance.now();
                frameTimes.push(t1 - t0);
            }

            return frameTimes;
        },
        { deltaX, deltaY, steps },
    );
}

async function benchmarkScroll(
    page: Page,
    label: string,
    deltaX: number,
    deltaY: number,
    warmupSteps: number,
    measuredSteps: number,
): Promise<{
    label: string;
    warmupAvgMs: number;
    avgMs: number;
    minMs: number;
    maxMs: number;
    p50Ms: number;
    p95Ms: number;
    p99Ms: number;
    allFrameTimes: number[];
}> {
    const warmupTimes = await scrollCanvas(page, deltaX, deltaY, warmupSteps);
    await new Promise<void>((r) => {
        setTimeout(r, 200);
    });

    const frameTimes = await scrollCanvas(page, deltaX, deltaY, measuredSteps);

    const avg = (arr: number[]) => arr.reduce((a, b) => a + b, 0) / arr.length;
    const sorted = [...frameTimes].sort((a, b) => a - b);
    const pct = (p: number) => sorted[Math.floor((p / 100) * sorted.length)];

    return {
        label,
        warmupAvgMs: avg(warmupTimes),
        avgMs: avg(frameTimes),
        minMs: sorted[0],
        maxMs: sorted[sorted.length - 1],
        p50Ms: pct(50),
        p95Ms: pct(95),
        p99Ms: pct(99),
        allFrameTimes: frameTimes,
    };
}

function formatMs(ms: number): string {
    return `${ms.toFixed(1)}ms`;
}

function printScrollBenchmark(
    b: ReturnType<typeof benchmarkScroll> extends Promise<infer T> ? T : never,
) {
    console.log(`  ${b.label}:`);
    console.log(`    Warmup avg:  ${formatMs(b.warmupAvgMs)}`);
    console.log(`    Avg:         ${formatMs(b.avgMs)}`);
    console.log(`    Min:         ${formatMs(b.minMs)}`);
    console.log(`    Max:         ${formatMs(b.maxMs)}`);
    console.log(`    P50:         ${formatMs(b.p50Ms)}`);
    console.log(`    P95:         ${formatMs(b.p95Ms)}`);
    console.log(`    P99:         ${formatMs(b.p99Ms)}`);
}

async function main() {
    if (!fs.existsSync(DIST_DIR)) {
        console.error(`dist not found: ${DIST_DIR}`);
        process.exit(1);
    }

    fs.mkdirSync(RESULTS_DIR, { recursive: true });

    console.log("Starting HTTPS perf server...");
    const server = await createServer();
    console.log(`HTTPS server running on https://localhost:${PORT}`);

    const results: PerfResult[] = [];

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

        const context = await browser.newContext({
            ignoreHTTPSErrors: true,
            viewport: { width: 1600, height: 900 },
        });

        const page = await context.newPage();
        const cdp = await context.newCDPSession(page);
        await cdp.send("Performance.enable");

        // --- Load page ---
        console.log("\n=== Loading page ===");
        const { durationMs: loadMs } = await measureTime("page load", () =>
            page.goto(`https://localhost:${PORT}/`, { waitUntil: "load" }),
        );
        console.log(`  Page load: ${formatMs(loadMs)}`);

        await page.waitForTimeout(1000);

        // ============================================================
        // LAZY LIST PERFORMANCE
        // ============================================================
        console.log("\n========== LAZY LIST PERFORMANCE ==========");

        // 1. Initial render time
        console.log("\n--- 1. Initial Render ---");
        const { durationMs: lazyInitMs } = await measureTime(
            "lazy-list initial render",
            () => selectCase(page, "lazy-list"),
        );
        await page.waitForTimeout(1000);

        const lazyInitElements = await countElements(page);
        const lazyInitMetrics = await getCDPMetrics(cdp);

        console.log(`  Initial render time: ${formatMs(lazyInitMs)}`);
        console.log(`  Element count: ${lazyInitElements}`);
        console.log(
            `  JS heap used: ${(lazyInitMetrics.JSHeapUsedSize / 1024 / 1024).toFixed(1)} MB`,
        );
        console.log(
            `  JS heap total: ${(lazyInitMetrics.JSHeapTotalSize / 1024 / 1024).toFixed(1)} MB`,
        );

        await screenshot(page, "lazy-init");

        results.push({
            name: "lazy-list-init",
            metrics: {
                renderTimeMs: lazyInitMs,
                elementCount: lazyInitElements,
                jsHeapUsedMB: lazyInitMetrics.JSHeapUsedSize / 1024 / 1024,
                jsHeapTotalMB: lazyInitMetrics.JSHeapTotalSize / 1024 / 1024,
            },
        });

        // 2. Scroll performance (vertical)
        console.log("\n--- 2. Vertical Scroll Performance ---");
        const lazyScrollBench = await benchmarkScroll(
            page,
            "LazyColumn scroll",
            0,
            120,
            5,
            50,
        );
        printScrollBenchmark(lazyScrollBench);
        await screenshot(page, "lazy-scroll-after");

        const lazyScrollElements = await countElements(page);
        console.log(`  Element count after scroll: ${lazyScrollElements}`);

        results.push({
            name: "lazy-list-scroll-vertical",
            metrics: {
                avgMs: lazyScrollBench.avgMs,
                p50Ms: lazyScrollBench.p50Ms,
                p95Ms: lazyScrollBench.p95Ms,
                p99Ms: lazyScrollBench.p99Ms,
                elementCount: lazyScrollElements,
            },
        });

        // 3. Switch to LazyRow
        console.log("\n--- 3. Tab Switch (Column -> Row) ---");
        await measureTime("tab switch", async () => {
            const layout = await page.evaluate(() => {
                const w = window as Record<string, unknown>;
                if (!w.turDemo) return "";
                return (
                    w.turDemo as { debugLayout: () => string }
                ).debugLayout();
            });
            return layout;
        });

        await page.evaluate(() => {
            const canvas = document.querySelector("#tur-container canvas");
            if (!canvas) return;
            const rect = canvas.getBoundingClientRect();
            const cx = rect.x + rect.width / 2;
            const cy = rect.y + rect.height / 2;

            canvas.dispatchEvent(
                new MouseEvent("mousedown", {
                    clientX: cx - 100,
                    clientY: cy - 380,
                    bubbles: true,
                }),
            );
            canvas.dispatchEvent(
                new MouseEvent("mouseup", {
                    clientX: cx - 100,
                    clientY: cy - 380,
                    bubbles: true,
                }),
            );
        });
        await page.waitForTimeout(500);

        // Actually click the Row tab via layout
        const rowTabClicked = await page.evaluate(() => {
            const w = window as Record<string, unknown>;
            if (!w.turDemo) return false;
            const layout = (
                w.turDemo as { debugLayout: () => string }
            ).debugLayout();
            if (!layout) return false;

            const lines = layout.split("\n");
            for (const line of lines) {
                if (line.includes('"Row"') && line.includes("abs(")) {
                    const match = line.match(
                        /abs\(([^,]+),([^)]+)\)\s+([\d.]+)x([\d.]+)/,
                    );
                    if (match) {
                        return {
                            x: parseFloat(match[1]) + parseFloat(match[3]) / 2,
                            y: parseFloat(match[2]) + parseFloat(match[4]) / 2,
                            w: parseFloat(match[3]),
                            h: parseFloat(match[4]),
                        };
                    }
                }
            }
            return false;
        });

        if (rowTabClicked && typeof rowTabClicked === "object") {
            const canvasOffset = await page.evaluate(() => {
                const canvas = document.querySelector("#tur-container canvas");
                if (!canvas) return { x: 0, y: 0 };
                const rect = canvas.getBoundingClientRect();
                return { x: rect.x, y: rect.y };
            });

            const px = Math.round(
                (rowTabClicked as { x: number; y: number }).x + canvasOffset.x,
            );
            const py = Math.round(
                (rowTabClicked as { x: number; y: number }).y + canvasOffset.y,
            );
            console.log(`  Clicking Row tab at (${px}, ${py})`);
            await page.mouse.click(px, py);
        }
        await page.waitForTimeout(1000);

        const lazyRowElements = await countElements(page);
        console.log(`  Element count after tab switch: ${lazyRowElements}`);
        await screenshot(page, "lazy-row");

        // 4. Horizontal scroll performance
        console.log("\n--- 4. Horizontal Scroll Performance ---");
        const lazyRowBench = await benchmarkScroll(
            page,
            "LazyRow scroll",
            120,
            0,
            5,
            50,
        );
        printScrollBenchmark(lazyRowBench);
        await screenshot(page, "lazy-row-scroll-after");

        const lazyRowScrollElements = await countElements(page);
        console.log(`  Element count after h-scroll: ${lazyRowScrollElements}`);

        results.push({
            name: "lazy-list-scroll-horizontal",
            metrics: {
                avgMs: lazyRowBench.avgMs,
                p50Ms: lazyRowBench.p50Ms,
                p95Ms: lazyRowBench.p95Ms,
                p99Ms: lazyRowBench.p99Ms,
                elementCount: lazyRowScrollElements,
            },
        });

        // ============================================================
        // SCROLL LIST PERFORMANCE (baseline comparison)
        // ============================================================
        console.log(
            "\n========== SCROLL LIST PERFORMANCE (baseline) ==========",
        );

        // 5. Initial render time
        console.log("\n--- 5. Initial Render ---");
        const { durationMs: scrollInitMs } = await measureTime(
            "scroll-list initial render",
            () => selectCase(page, "scroll-list"),
        );
        await page.waitForTimeout(1000);

        const scrollInitElements = await countElements(page);
        const scrollInitMetrics = await getCDPMetrics(cdp);

        console.log(`  Initial render time: ${formatMs(scrollInitMs)}`);
        console.log(`  Element count: ${scrollInitElements}`);
        console.log(
            `  JS heap used: ${(scrollInitMetrics.JSHeapUsedSize / 1024 / 1024).toFixed(1)} MB`,
        );
        console.log(
            `  JS heap total: ${(scrollInitMetrics.JSHeapTotalSize / 1024 / 1024).toFixed(1)} MB`,
        );

        await screenshot(page, "scroll-init");

        results.push({
            name: "scroll-list-init",
            metrics: {
                renderTimeMs: scrollInitMs,
                elementCount: scrollInitElements,
                jsHeapUsedMB: scrollInitMetrics.JSHeapUsedSize / 1024 / 1024,
                jsHeapTotalMB: scrollInitMetrics.JSHeapTotalSize / 1024 / 1024,
            },
        });

        // 6. Scroll performance
        console.log("\n--- 6. Vertical Scroll Performance ---");
        const scrollBench = await benchmarkScroll(
            page,
            "ScrollView scroll",
            0,
            120,
            5,
            50,
        );
        printScrollBenchmark(scrollBench);
        await screenshot(page, "scroll-scroll-after");

        const scrollScrollElements = await countElements(page);
        console.log(`  Element count after scroll: ${scrollScrollElements}`);

        results.push({
            name: "scroll-list-scroll",
            metrics: {
                avgMs: scrollBench.avgMs,
                p50Ms: scrollBench.p50Ms,
                p95Ms: scrollBench.p95Ms,
                p99Ms: scrollBench.p99Ms,
                elementCount: scrollScrollElements,
            },
        });

        // ============================================================
        // COMPARISON
        // ============================================================
        console.log("\n========== COMPARISON ==========");
        console.log("\n  Render Time (initial load):");
        console.log(
            `    LazyColumn:  ${formatMs(results.find((r) => r.name === "lazy-list-init")?.metrics.renderTimeMs ?? 0)}`,
        );
        console.log(
            `    ScrollView:  ${formatMs(results.find((r) => r.name === "scroll-list-init")?.metrics.renderTimeMs ?? 0)}`,
        );

        console.log("\n  Element Count (initial):");
        console.log(
            `    LazyColumn:  ${results.find((r) => r.name === "lazy-list-init")?.metrics.elementCount}`,
        );
        console.log(
            `    ScrollView:  ${results.find((r) => r.name === "scroll-list-init")?.metrics.elementCount}`,
        );

        console.log("\n  Element Count (after scroll):");
        console.log(
            `    LazyColumn:  ${lazyScrollElements} (virtualized, should stay ~same)`,
        );
        console.log(
            `    ScrollView:  ${scrollScrollElements} (all items, should stay ~same)`,
        );

        console.log("\n  Scroll Latency (vertical, avg per frame):");
        console.log(`    LazyColumn:  ${formatMs(lazyScrollBench.avgMs)}`);
        console.log(`    ScrollView:  ${formatMs(scrollBench.avgMs)}`);
        const speedup = scrollBench.avgMs / lazyScrollBench.avgMs;
        console.log(
            `    Ratio:        ${speedup.toFixed(2)}x (${speedup > 1 ? "lazy faster" : "scroll faster"})`,
        );

        console.log("\n  Scroll Latency P95:");
        console.log(`    LazyColumn:  ${formatMs(lazyScrollBench.p95Ms)}`);
        console.log(`    ScrollView:  ${formatMs(scrollBench.p95Ms)}`);

        console.log("\n  JS Heap Used:");
        console.log(
            `    LazyColumn:  ${results.find((r) => r.name === "lazy-list-init")?.metrics.jsHeapUsedMB?.toFixed(1)} MB`,
        );
        console.log(
            `    ScrollView:  ${results.find((r) => r.name === "scroll-list-init")?.metrics.jsHeapUsedMB?.toFixed(1)} MB`,
        );

        console.log("\n  LazyRow Horizontal Scroll:");
        console.log(`    Avg: ${formatMs(lazyRowBench.avgMs)}`);
        console.log(`    P95: ${formatMs(lazyRowBench.p95Ms)}`);

        // Write JSON results
        const resultsPath = path.join(RESULTS_DIR, "perf-results.json");
        fs.writeFileSync(resultsPath, JSON.stringify(results, null, 2));
        console.log(`\n  Results saved to: ${resultsPath}`);

        // ============================================================
        // RAPID SCROLL STRESS TEST
        // ============================================================
        console.log("\n========== RAPID SCROLL STRESS TEST ==========");

        // Back to lazy-list
        await selectCase(page, "lazy-list");
        await page.waitForTimeout(1000);

        console.log("\n--- 7. Rapid 200-step scroll (LazyColumn) ---");
        const lazyStress = await benchmarkScroll(
            page,
            "LazyColumn stress",
            0,
            120,
            10,
            200,
        );
        console.log(`  Avg: ${formatMs(lazyStress.avgMs)}`);
        console.log(`  P95: ${formatMs(lazyStress.p95Ms)}`);
        console.log(`  P99: ${formatMs(lazyStress.p99Ms)}`);
        console.log(`  Max: ${formatMs(lazyStress.maxMs)}`);
        const framesAbove16ms = lazyStress.allFrameTimes.filter(
            (t) => t > 16,
        ).length;
        const framesAbove33ms = lazyStress.allFrameTimes.filter(
            (t) => t > 33,
        ).length;
        console.log(
            `  Frames >16ms: ${framesAbove16ms}/${lazyStress.allFrameTimes.length} (${((framesAbove16ms / lazyStress.allFrameTimes.length) * 100).toFixed(1)}%)`,
        );
        console.log(
            `  Frames >33ms: ${framesAbove33ms}/${lazyStress.allFrameTimes.length} (${((framesAbove33ms / lazyStress.allFrameTimes.length) * 100).toFixed(1)}%)`,
        );

        await screenshot(page, "lazy-stress-after");

        // Switch to scroll-list
        await selectCase(page, "scroll-list");
        await page.waitForTimeout(1000);

        console.log("\n--- 8. Rapid 200-step scroll (ScrollView) ---");
        const scrollStress = await benchmarkScroll(
            page,
            "ScrollView stress",
            0,
            120,
            10,
            200,
        );
        console.log(`  Avg: ${formatMs(scrollStress.avgMs)}`);
        console.log(`  P95: ${formatMs(scrollStress.p95Ms)}`);
        console.log(`  P99: ${formatMs(scrollStress.p99Ms)}`);
        console.log(`  Max: ${formatMs(scrollStress.maxMs)}`);
        const scrollFramesAbove16ms = scrollStress.allFrameTimes.filter(
            (t) => t > 16,
        ).length;
        const scrollFramesAbove33ms = scrollStress.allFrameTimes.filter(
            (t) => t > 33,
        ).length;
        console.log(
            `  Frames >16ms: ${scrollFramesAbove16ms}/${scrollStress.allFrameTimes.length} (${((scrollFramesAbove16ms / scrollStress.allFrameTimes.length) * 100).toFixed(1)}%)`,
        );
        console.log(
            `  Frames >33ms: ${scrollFramesAbove33ms}/${scrollStress.allFrameTimes.length} (${((scrollFramesAbove33ms / scrollStress.allFrameTimes.length) * 100).toFixed(1)}%)`,
        );

        await screenshot(page, "scroll-stress-after");

        console.log("\n  Stress Test Comparison:");
        console.log(`    LazyColumn avg: ${formatMs(lazyStress.avgMs)}`);
        console.log(`    ScrollView avg: ${formatMs(scrollStress.avgMs)}`);
        console.log(
            `    Ratio: ${(scrollStress.avgMs / lazyStress.avgMs).toFixed(2)}x`,
        );
        console.log(
            `    LazyColumn >16ms: ${framesAbove16ms}/${lazyStress.allFrameTimes.length}`,
        );
        console.log(
            `    ScrollView >16ms: ${scrollFramesAbove16ms}/${scrollStress.allFrameTimes.length}`,
        );

        await browser.close();
    } finally {
        server.close();
    }
}

main().catch((err) => {
    console.error(err);
    process.exit(1);
});
