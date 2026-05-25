import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const RESULTS_DIR = path.resolve(__dirname, "../test-results");
const DEPLOY_URL = process.env.DEPLOY_URL || "https://tur-react-demo.pages.dev";

interface ConsoleEntry {
    type: string;
    text: string;
}

async function main() {
    fs.mkdirSync(RESULTS_DIR, { recursive: true });

    for (const file of fs.readdirSync(RESULTS_DIR)) {
        if (file.endsWith(".png")) {
            fs.unlinkSync(path.join(RESULTS_DIR, file));
        }
    }

    console.log(`Visiting ${DEPLOY_URL} ...`);

    const logs: ConsoleEntry[] = [];

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
        viewport: { width: 800, height: 600 },
    });

    const page = await context.newPage();

    page.on("console", (msg) => {
        logs.push({ type: msg.type(), text: msg.text() });
    });

    page.on("pageerror", (err) => {
        logs.push({
            type: "error",
            text: `[PAGE_ERROR] ${err.message}\n${err.stack}`,
        });
    });

    await page.goto(DEPLOY_URL, { waitUntil: "load", timeout: 30000 });

    const gpuInfo = await page.evaluate(async () => {
        if (!navigator.gpu)
            return { supported: false, reason: "navigator.gpu not available" };
        try {
            const adapter = await navigator.gpu.requestAdapter();
            if (!adapter)
                return { supported: false, reason: "no adapter returned" };
            const info = adapter.info as Record<string, string>;
            return {
                supported: true,
                vendor: info.vendor,
                architecture: info.architecture,
            };
        } catch (e: unknown) {
            const msg = e instanceof Error ? e.message : String(e);
            return { supported: false, reason: msg };
        }
    });
    console.log("WebGPU:", JSON.stringify(gpuInfo));

    await page.waitForTimeout(5000);

    await page.screenshot({
        path: path.join(RESULTS_DIR, "01-prod.png"),
        fullPage: true,
    });
    console.log("Screenshot saved to test-results/01-prod.png");

    console.log("\n=== Browser Console ===");
    for (const entry of logs) {
        const prefix =
            entry.type === "error"
                ? "ERR"
                : entry.type === "warning"
                  ? "WRN"
                  : "LOG";
        console.log(`[${prefix}] ${entry.text}`);
    }
    console.log("=== End Console ===");

    await browser.close();
}

main().catch((err) => {
    console.error(err);
    process.exit(1);
});
