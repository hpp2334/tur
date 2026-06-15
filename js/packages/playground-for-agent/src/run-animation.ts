import fs from "node:fs";
import https from "node:https";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { chromium, type Page } from "playwright";
import { generate as generateCert } from "selfsigned";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const DIST_DIR = path.resolve(__dirname, "../../tur-demo/dist");
const RESULTS_DIR = path.resolve(__dirname, "../test-results");
const PORT = 3999;

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

let stepCounter = 0;

async function screenshot(page: Page, label: string) {
    stepCounter++;
    const name = String(stepCounter).padStart(2, "0");
    const filePath = path.join(RESULTS_DIR, `${name}-${label}.png`);
    await page.screenshot({ path: filePath, fullPage: true });
    console.log(`Screenshot [${name}]: ${filePath}`);
    return filePath;
}

async function main() {
    if (!fs.existsSync(DIST_DIR)) {
        console.error(`dist not found: ${DIST_DIR}`);
        console.error("Run: cd js/packages/tur-demo && pnpm build");
        process.exit(1);
    }

    fs.mkdirSync(RESULTS_DIR, { recursive: true });
    for (const file of fs.readdirSync(RESULTS_DIR)) {
        if (file.endsWith(".png")) fs.unlinkSync(path.join(RESULTS_DIR, file));
    }

    console.log("Starting HTTPS server...");
    const server = await createServer();
    console.log(`HTTPS server running on https://localhost:${PORT}`);

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

        page.on("console", (msg) => {
            if (msg.type() === "error") console.log(`[ERR] ${msg.text()}`);
        });

        page.on("pageerror", (err) => {
            console.log(`[PAGE_ERROR] ${err.message}`);
        });

        // --- Step 1: Load page ---
        console.log("\n--- Step 1: Load page ---");
        await page.goto(`https://localhost:${PORT}/`, { waitUntil: "load" });
        await page.waitForTimeout(1000);

        const gpuInfo = await page.evaluate(async () => {
            if (!navigator.gpu) return { supported: false };
            try {
                const adapter = await navigator.gpu.requestAdapter();
                return { supported: !!adapter };
            } catch {
                return { supported: false };
            }
        });
        console.log("WebGPU:", JSON.stringify(gpuInfo));

        // --- Step 2: Select animation-basic case ---
        console.log("\n--- Step 2: Select animation-basic case ---");
        const animBtn = page.locator("button.case-item", {
            hasText: /^animation-basic$/,
        });
        const count = await animBtn.count();
        if (count === 0) {
            console.log("FAIL: animation-basic case not found in sidebar");
            await browser.close();
            server.close();
            process.exit(1);
        }
        await animBtn.click();

        await page.waitForFunction(
            () => {
                const w = window as Record<string, unknown>;
                if (!w.turDemo) return false;
                try {
                    const layout = (
                        w.turDemo as { debugLayout: () => string }
                    ).debugLayout();
                    return typeof layout === "string" && layout.length > 50;
                } catch {
                    return false;
                }
            },
            { timeout: 30000 },
        );
        await page.waitForTimeout(1500);

        await screenshot(page, "animation-initial");

        // Get layout
        const layout = await page.evaluate(() => {
            const w = window as Record<string, unknown>;
            if (!w.turDemo) return "";
            return (w.turDemo as { debugLayout: () => string }).debugLayout();
        });
        console.log(`Layout length: ${layout.length}`);

        // Get canvas offset for clicking
        const canvasOffset = await page.evaluate(() => {
            const canvas = document.querySelector("#tur-container canvas");
            if (!canvas) return { x: 0, y: 0 };
            const rect = canvas.getBoundingClientRect();
            return { x: rect.x, y: rect.y };
        });
        console.log(`Canvas offset: ${JSON.stringify(canvasOffset)}`);

        // --- Step 3: Click "Size" button to animate ---
        console.log("\n--- Step 3: Click Size button ---");
        // Find the Size button in the canvas layout
        const sizeButtonPos = await page.evaluate(() => {
            const w = window as Record<string, unknown>;
            if (!w.turDemo) return null;
            const layout = (
                w.turDemo as { debugLayout: () => string }
            ).debugLayout();
            const lines = layout.split("\n");
            for (const line of lines) {
                if (line.includes('"Size"')) {
                    const absMatch = line.match(
                        /abs\(([^,]+),([^)]+)\)\s+([\d.]+)x([\d.]+)/,
                    );
                    if (absMatch) {
                        return {
                            x: parseFloat(absMatch[1]),
                            y: parseFloat(absMatch[2]),
                            w: parseFloat(absMatch[3]),
                            h: parseFloat(absMatch[4]),
                        };
                    }
                }
            }
            return null;
        });
        console.log("Size button position:", JSON.stringify(sizeButtonPos));

        if (sizeButtonPos) {
            const px = Math.round(
                sizeButtonPos.x + sizeButtonPos.w / 2 + canvasOffset.x,
            );
            const py = Math.round(
                sizeButtonPos.y + sizeButtonPos.h / 2 + canvasOffset.y,
            );
            console.log(`Clicking Size at (${px}, ${py})`);
            await page.mouse.click(px, py);
            await page.waitForTimeout(800);
        }

        await screenshot(page, "animation-size-toggled");

        // Wait for animation to finish
        await page.waitForTimeout(1000);
        await screenshot(page, "animation-size-done");

        // --- Step 4: Click "Color" button ---
        console.log("\n--- Step 4: Click Color button ---");
        const colorButtonPos = await page.evaluate(() => {
            const w = window as Record<string, unknown>;
            if (!w.turDemo) return null;
            const layout = (
                w.turDemo as { debugLayout: () => string }
            ).debugLayout();
            const lines = layout.split("\n");
            for (const line of lines) {
                if (line.includes('"Color"')) {
                    const absMatch = line.match(
                        /abs\(([^,]+),([^)]+)\)\s+([\d.]+)x([\d.]+)/,
                    );
                    if (absMatch) {
                        return {
                            x: parseFloat(absMatch[1]),
                            y: parseFloat(absMatch[2]),
                            w: parseFloat(absMatch[3]),
                            h: parseFloat(absMatch[4]),
                        };
                    }
                }
            }
            return null;
        });
        console.log("Color button position:", JSON.stringify(colorButtonPos));

        if (colorButtonPos) {
            const px = Math.round(
                colorButtonPos.x + colorButtonPos.w / 2 + canvasOffset.x,
            );
            const py = Math.round(
                colorButtonPos.y + colorButtonPos.h / 2 + canvasOffset.y,
            );
            console.log(`Clicking Color at (${px}, ${py})`);
            await page.mouse.click(px, py);
            await page.waitForTimeout(800);
        }

        await screenshot(page, "animation-color-change");

        // Wait for animation
        await page.waitForTimeout(1000);
        await screenshot(page, "animation-color-done");

        // --- Step 5: Click "Move" button ---
        console.log("\n--- Step 5: Click Move button ---");
        const moveButtonPos = await page.evaluate(() => {
            const w = window as Record<string, unknown>;
            if (!w.turDemo) return null;
            const layout = (
                w.turDemo as { debugLayout: () => string }
            ).debugLayout();
            const lines = layout.split("\n");
            for (const line of lines) {
                if (line.includes('"Move"')) {
                    const absMatch = line.match(
                        /abs\(([^,]+),([^)]+)\)\s+([\d.]+)x([\d.]+)/,
                    );
                    if (absMatch) {
                        return {
                            x: parseFloat(absMatch[1]),
                            y: parseFloat(absMatch[2]),
                            w: parseFloat(absMatch[3]),
                            h: parseFloat(absMatch[4]),
                        };
                    }
                }
            }
            return null;
        });
        console.log("Move button position:", JSON.stringify(moveButtonPos));

        if (moveButtonPos) {
            const px = Math.round(
                moveButtonPos.x + moveButtonPos.w / 2 + canvasOffset.x,
            );
            const py = Math.round(
                moveButtonPos.y + moveButtonPos.h / 2 + canvasOffset.y,
            );
            console.log(`Clicking Move at (${px}, ${py})`);
            await page.mouse.click(px, py);
            await page.waitForTimeout(800);
        }

        await screenshot(page, "animation-move");

        // Wait for animation
        await page.waitForTimeout(1000);
        await screenshot(page, "animation-move-done");

        // --- Step 6: Change curve to linear ---
        console.log("\n--- Step 6: Click linear curve button ---");
        const linearBtnPos = await page.evaluate(() => {
            const w = window as Record<string, unknown>;
            if (!w.turDemo) return null;
            const layout = (
                w.turDemo as { debugLayout: () => string }
            ).debugLayout();
            const lines = layout.split("\n");
            for (const line of lines) {
                if (line.includes('"linear"')) {
                    const absMatch = line.match(
                        /abs\(([^,]+),([^)]+)\)\s+([\d.]+)x([\d.]+)/,
                    );
                    if (absMatch) {
                        return {
                            x: parseFloat(absMatch[1]),
                            y: parseFloat(absMatch[2]),
                            w: parseFloat(absMatch[3]),
                            h: parseFloat(absMatch[4]),
                        };
                    }
                }
            }
            return null;
        });
        console.log("linear button position:", JSON.stringify(linearBtnPos));

        if (linearBtnPos) {
            const px = Math.round(
                linearBtnPos.x + linearBtnPos.w / 2 + canvasOffset.x,
            );
            const py = Math.round(
                linearBtnPos.y + linearBtnPos.h / 2 + canvasOffset.y,
            );
            console.log(`Clicking linear at (${px}, ${py})`);
            await page.mouse.click(px, py);
            await page.waitForTimeout(300);
        }

        // Click Size again with linear curve
        if (sizeButtonPos) {
            const px = Math.round(
                sizeButtonPos.x + sizeButtonPos.w / 2 + canvasOffset.x,
            );
            const py = Math.round(
                sizeButtonPos.y + sizeButtonPos.h / 2 + canvasOffset.y,
            );
            await page.mouse.click(px, py);
            await page.waitForTimeout(800);
        }

        await screenshot(page, "animation-linear-size");

        // --- Step 7: Change duration to 1500ms ---
        console.log("\n--- Step 7: Click 1500ms duration button ---");
        const dur1500BtnPos = await page.evaluate(() => {
            const w = window as Record<string, unknown>;
            if (!w.turDemo) return null;
            const layout = (
                w.turDemo as { debugLayout: () => string }
            ).debugLayout();
            const lines = layout.split("\n");
            for (const line of lines) {
                if (line.includes('"1500"')) {
                    const absMatch = line.match(
                        /abs\(([^,]+),([^)]+)\)\s+([\d.]+)x([\d.]+)/,
                    );
                    if (absMatch) {
                        return {
                            x: parseFloat(absMatch[1]),
                            y: parseFloat(absMatch[2]),
                            w: parseFloat(absMatch[3]),
                            h: parseFloat(absMatch[4]),
                        };
                    }
                }
            }
            return null;
        });
        console.log("1500ms button position:", JSON.stringify(dur1500BtnPos));

        if (dur1500BtnPos) {
            const px = Math.round(
                dur1500BtnPos.x + dur1500BtnPos.w / 2 + canvasOffset.x,
            );
            const py = Math.round(
                dur1500BtnPos.y + dur1500BtnPos.h / 2 + canvasOffset.y,
            );
            console.log(`Clicking 1500 at (${px}, ${py})`);
            await page.mouse.click(px, py);
            await page.waitForTimeout(300);
        }

        // Click Color to trigger slow animation
        if (colorButtonPos) {
            const px = Math.round(
                colorButtonPos.x + colorButtonPos.w / 2 + canvasOffset.x,
            );
            const py = Math.round(
                colorButtonPos.y + colorButtonPos.h / 2 + canvasOffset.y,
            );
            await page.mouse.click(px, py);
        }

        // Screenshot during slow animation
        await page.waitForTimeout(400);
        await screenshot(page, "animation-slow-mid");

        // Wait for full completion
        await page.waitForTimeout(1200);
        await screenshot(page, "animation-slow-done");

        console.log("\n=== Animation test complete ===");

        await browser.close();
    } catch (err) {
        console.error("Test failed:", err);
        process.exit(1);
    } finally {
        server.close();
    }
}

main();
