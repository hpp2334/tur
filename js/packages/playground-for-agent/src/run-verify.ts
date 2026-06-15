import fs from "node:fs";
import https from "node:https";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";
import { generate as generateCert } from "selfsigned";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const DIST_DIR = path.resolve(__dirname, "../../tur-demo/dist");
const RESULTS_DIR = path.resolve(__dirname, "../test-results");
fs.mkdirSync(RESULTS_DIR, { recursive: true });
const PORT = 3999;

const MIME: Record<string, string> = {
    ".html": "text/html",
    ".js": "text/javascript",
    ".mjs": "text/javascript",
    ".css": "text/css",
    ".wasm": "application/wasm",
    ".json": "application/json",
    ".png": "image/png",
    ".svg": "image/svg+xml",
};

async function main() {
    const attrs = [{ name: "commonName", value: "localhost" }];
    const opts = {
        days: 365,
        algorithm: "sha256" as const,
        extensions: [
            {
                name: "subjectAltName",
                altNames: [{ type: 2, value: "localhost" }],
            },
        ],
    };
    const { cert, private: key } = await generateCert(attrs, opts);

    const server = https.createServer({ cert, key }, (req, res) => {
        const urlPath = req.url === "/" ? "/index.html" : (req.url as string);
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
            "Content-Type": MIME[ext] || "application/octet-stream",
            "Content-Length": content.length,
            "Cross-Origin-Opener-Policy": "same-origin",
            "Cross-Origin-Embedder-Policy": "require-corp",
        });
        res.end(content);
    });

    await new Promise<void>((resolve) => server.listen(PORT, resolve));
    console.log(`Server on https://localhost:${PORT}`);

    const browser = await chromium.launch({
        headless: true,
        args: [
            "--ignore-certificate-errors",
            "--enable-unsafe-webgpu",
            "--use-angle=metal",
            "--ignore-gpu-blocklist",
            "--disable-software-rasterizer",
        ],
    });
    const context = await browser.newContext({
        viewport: { width: 1280, height: 900 },
        ignoreHTTPSErrors: true,
    });
    const page = await context.newPage();
    page.on("console", (msg) => {
        const t = msg.type();
        console.log(`[browser ${t}]`, msg.text());
    });
    page.on("pageerror", (err) => {
        console.log("[browser pageerror]", err.message);
    });

    await page.goto(`https://localhost:${PORT}/`, {
        waitUntil: "domcontentloaded",
        timeout: 60000,
    });

    // Wait for case buttons to appear.
    await page.waitForSelector("button.case-item", { timeout: 30000 });
    console.log("Page loaded; cases visible.");

    // Probe the initial state of globalThis.
    const probe = await page.evaluate(() => {
        const w = window as Record<string, unknown>;
        return {
            hasTurDemo: !!w.turDemo,
            hasTur: !!(w as Record<string, unknown>).__tur,
            hasTurEdgy: !!w.TurEdgy,
            caseButtons: document.querySelectorAll("button.case-item").length,
        };
    });
    console.log("Initial probe:", probe);

    // 1) Verify counter case loads and renders.
    console.log("\n=== counter case ===");
    await page.locator("button.case-item", { hasText: /^counter$/ }).click();
    // Wait for the wasm to actually load and render the bundle. We poll the
    // layout string until it's non-empty (or timeout).
    let layout = "";
    for (let i = 0; i < 60; i++) {
        await page.waitForTimeout(500);
        layout = await page.evaluate(() => {
            const w = window as Record<string, unknown>;
            const turApp = w.turApp as
                | { debug_layout: () => string }
                | undefined;
            return turApp?.debug_layout?.() ?? "";
        });
        if (layout.length > 0) break;
    }
    console.log("counter layout (after wait):", layout.substring(0, 500));

    // Check the canvas.
    const canvasInfo = await page.evaluate(() => {
        const canvas = document.querySelector(
            "#tur-container canvas",
        ) as HTMLCanvasElement | null;
        if (!canvas) return { found: false };
        const r = canvas.getBoundingClientRect();
        return {
            found: true,
            width: canvas.width,
            height: canvas.height,
            rect: { x: r.x, y: r.y, w: r.width, h: r.height },
            ctx: (() => {
                try {
                    const ctx = canvas.getContext("2d");
                    return ctx ? "2d" : "no-2d";
                } catch {
                    return "exception";
                }
            })(),
        };
    });
    console.log("canvas info:", JSON.stringify(canvasInfo));
    await page.screenshot({ path: path.join(RESULTS_DIR, "counter.png") });
    console.log("  screenshot saved: counter.png");

    // Click +1 a few times.
    const canvas = await page.evaluate(() => {
        const el = document.querySelector(
            "#tur-container canvas",
        ) as HTMLCanvasElement | null;
        if (!el) return null;
        const r = el.getBoundingClientRect();
        return { x: r.x, y: r.y, w: r.width, h: r.height };
    });
    if (canvas) {
        console.log(`  canvas at ${JSON.stringify(canvas)}`);
        // Find the +1 button — it's centered, lower half. Click in the center.
        // First verify there's text "Count: 0".
        const initial = await page.evaluate(() => {
            const w = window as Record<string, unknown>;
            return (w.turDemo as { debugLayout: () => string }).debugLayout();
        });
        console.log(
            "  initial layout snippet:",
            initial.split("\n").slice(0, 6).join(" | "),
        );

        // Click roughly where +1 should be (center-bottom).
        const cx = canvas.x + canvas.w / 2;
        const cy = canvas.y + canvas.h / 2 + 50;
        await page.mouse.click(cx, cy);
        await page.waitForTimeout(400);
        await page.mouse.click(cx, cy);
        await page.waitForTimeout(400);
        await page.screenshot({
            path: path.join(RESULTS_DIR, "counter-clicked.png"),
        });
        console.log("  screenshot saved: counter-clicked.png");

        const after = await page.evaluate(() => {
            const w = window as Record<string, unknown>;
            return (w.turDemo as { debugLayout: () => string }).debugLayout();
        });
        console.log(
            "  after-click layout snippet:",
            after.split("\n").slice(0, 6).join(" | "),
        );
    }

    // 2) Verify todolist case loads.
    console.log("\n=== todolist case ===");
    await page.locator("button.case-item", { hasText: /^todolist$/ }).click();
    await page.waitForFunction(
        () => {
            const w = window as Record<string, unknown>;
            if (!w.turDemo) return false;
            try {
                const layout = (
                    w.turDemo as { debugLayout: () => string }
                ).debugLayout();
                return (
                    typeof layout === "string" && layout.includes("Todolist")
                );
            } catch {
                return false;
            }
        },
        { timeout: 30000 },
    );
    await page.waitForTimeout(800);
    await page.screenshot({ path: path.join(RESULTS_DIR, "todolist.png") });
    console.log("  screenshot saved: todolist.png");

    // 3) Verify clickable-text case loads.
    console.log("\n=== clickable-text case ===");
    await page
        .locator("button.case-item", { hasText: /^clickable-text$/ })
        .click();
    await page.waitForFunction(
        () => {
            const w = window as Record<string, unknown>;
            if (!w.turDemo) return false;
            try {
                const layout = (
                    w.turDemo as { debugLayout: () => string }
                ).debugLayout();
                return typeof layout === "string" && layout.includes("before");
            } catch {
                return false;
            }
        },
        { timeout: 30000 },
    );
    await page.waitForTimeout(500);
    await page.screenshot({
        path: path.join(RESULTS_DIR, "clickable-text.png"),
    });
    console.log("  screenshot saved: clickable-text.png");

    // Click the text — should change to "after".
    const offset = await page.evaluate(() => {
        const el = document.querySelector(
            "#tur-container canvas",
        ) as HTMLCanvasElement | null;
        if (!el) return null;
        const r = el.getBoundingClientRect();
        return { x: r.x, y: r.y };
    });
    if (offset) {
        await page.mouse.click(offset.x + 50, offset.y + 30);
        await page.waitForTimeout(500);
        await page.screenshot({
            path: path.join(RESULTS_DIR, "clickable-text-clicked.png"),
        });
        const after = await page.evaluate(() => {
            const w = window as Record<string, unknown>;
            return (w.turDemo as { debugLayout: () => string }).debugLayout();
        });
        const changed = after.includes("after");
        console.log(`  click triggered text update: ${changed ? "YES" : "NO"}`);
    }

    await browser.close();
    server.close();
    console.log("\nDone.");
}

main().catch((e) => {
    console.error(e);
    process.exit(1);
});
