import https from "node:https";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { generate as generateCert } from "selfsigned";
import { chromium, type Page } from "playwright";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const DIST_DIR = path.resolve(__dirname, "../../tur-react-demo/dist");
const RESULTS_DIR = path.resolve(__dirname, "../test-results");
const PORT = 3999;

const MIME_TYPES: Record<string, string> = {
  ".html": "text/html",
  ".js": "text/javascript",
  ".wasm": "application/wasm",
  ".bin": "text/plain",
  ".map": "application/json",
  ".d.ts": "text/typescript",
};

async function createServer(): Promise<https.Server> {
  const attrs = [{ name: "commonName", value: "localhost" }];
  const options = {
    days: 365,
    algorithm: "sha256" as const,
    extensions: [
      { name: "subjectAltName", altNames: [{ type: 2, value: "localhost" }] },
    ],
  };
  const { cert, private: key } = await generateCert(attrs, options);

  return new Promise((resolve, reject) => {
    const server = https.createServer({ cert, key }, (req, res) => {
      const urlPath = req.url === "/" ? "/index.html" : req.url!;
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
      res.writeHead(200, {
        "Content-Type": contentType,
        "Content-Length": content.length,
      });
      res.end(content);
    });

    server.listen(PORT, () => resolve(server));
    server.on("error", reject);
  });
}

interface ConsoleEntry {
  type: string;
  text: string;
}

interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

interface ElementInfo {
  type: string;
  label: string;
  rect: Rect;
}

function parseLayout(layout: string): ElementInfo[] {
  const elements: ElementInfo[] = [];
  for (const line of layout.split("\n")) {
    const absMatch = line.match(/abs\(([^,]+),([^)]+)\)\s+([\d.]+)x([\d.]+)/);
    if (!absMatch) continue;
    const [, x, y, w, h] = absMatch;
    const typeMatch = line.match(/\b(tur_\S+)/);
    const type = typeMatch ? typeMatch[1] : "unknown";
    const labelMatch = line.match(/"([^"]*)"/);
    const label = labelMatch ? labelMatch[1] : "";
    elements.push({
      type,
      label,
      rect: {
        x: parseFloat(x),
        y: parseFloat(y),
        w: parseFloat(w),
        h: parseFloat(h),
      },
    });
  }
  return elements;
}

function center(rect: Rect): { x: number; y: number } {
  return { x: rect.x + rect.w / 2, y: rect.y + rect.h / 2 };
}

function findElement(
  elements: ElementInfo[],
  predicate: (e: ElementInfo) => boolean,
): ElementInfo | undefined {
  return elements.find(predicate);
}

function findElementByText(
  elements: ElementInfo[],
  text: string,
): ElementInfo | undefined {
  return findElement(elements, (e) => e.label === text && e.type === "tur_text_span");
}

function findClickTargetForText(
  elements: ElementInfo[],
  text: string,
): { x: number; y: number } | null {
  const span = findElementByText(elements, text);
  if (!span) return null;
  const sx = span.rect.x;
  const sy = span.rect.y;
  const parent = findElement(
    elements,
    (e) =>
      e.type === "tur_pointer_interact" &&
      e.rect.x <= sx &&
      e.rect.y <= sy &&
      e.rect.x + e.rect.w >= sx &&
      e.rect.y + e.rect.h >= sy,
  );
  if (parent) return center(parent.rect);
  return { x: sx, y: sy };
}

function findDeleteButtonForItem(
  elements: ElementInfo[],
  itemText: string,
): { x: number; y: number } | null {
  const span = findElementByText(elements, itemText);
  if (!span) return null;
  const sy = span.rect.y;
  const delRow = findElement(
    elements,
    (e) =>
      e.type === "tur_pointer_interact" &&
      e.label === "" &&
      e.rect.h <= 40 &&
      Math.abs(e.rect.y - sy) < 30 &&
      e.rect.x > 700,
  );
  if (delRow) return center(delRow.rect);
  return null;
}

function findCheckboxForItem(
  elements: ElementInfo[],
  itemText: string,
): { x: number; y: number } | null {
  const span = findElementByText(elements, itemText);
  if (!span) return null;
  const sy = span.rect.y;
  const checkbox = findElement(
    elements,
    (e) =>
      e.type === "tur_pointer_interact" &&
      e.rect.w <= 30 &&
      e.rect.h <= 30 &&
      Math.abs(e.rect.y - sy) < 20 &&
      e.rect.x < span.rect.x,
  );
  if (checkbox) return center(checkbox.rect);
  return null;
}

function findItemRow(
  elements: ElementInfo[],
  itemText: string,
): { x: number; y: number } | null {
  const span = findElementByText(elements, itemText);
  if (!span) return null;
  const sy = span.rect.y;
  const row = findElement(
    elements,
    (e) =>
      e.type === "tur_pointer_interact" &&
      e.rect.w > 400 &&
      Math.abs(e.rect.y - sy) < 30 &&
      e.rect.x < span.rect.x,
  );
  if (row) return center(row.rect);
  return null;
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

async function waitForRender(page: Page, ms = 500) {
  await page.waitForTimeout(ms);
}

async function getLayout(page: Page): Promise<ElementInfo[]> {
  const layout: string | undefined = await page.evaluate(() => {
    const w = window as any;
    if (!w.turDemo) return "";
    const result = w.turDemo.debugLayout();
    return typeof result === "string" ? result : JSON.stringify(result);
  });
  const elements = parseLayout(layout || "");
  console.log(`  Parsed ${elements.length} elements from layout (${(layout || "").length} chars)`);
  return elements;
}

function click(label: string, target: { x: number; y: number } | null) {
  if (!target) {
    console.log(`  WARNING: could not find "${label}" — skipping click`);
    return false;
  }
  console.log(`  Clicking "${label}" at (${Math.round(target.x)}, ${Math.round(target.y)})`);
  return true;
}

async function printConsole(logs: ConsoleEntry[]) {
  console.log("\n=== Browser Console ===");
  for (const entry of logs) {
    const prefix = entry.type === "error" ? "ERR" : entry.type === "warning" ? "WRN" : "LOG";
    console.log(`[${prefix}] ${entry.text}`);
  }
  console.log("=== End Console ===\n");
}

async function main() {
  if (!fs.existsSync(DIST_DIR)) {
    console.error(`dist not found: ${DIST_DIR}`);
    console.error("Run: cd js/packages/tur-react-demo && pnpm build");
    process.exit(1);
  }

  fs.mkdirSync(RESULTS_DIR, { recursive: true });

  for (const file of fs.readdirSync(RESULTS_DIR)) {
    if (file.endsWith(".png")) {
      fs.unlinkSync(path.join(RESULTS_DIR, file));
    }
  }

  console.log("Starting HTTPS server...");
  const server = await createServer();
  console.log(`HTTPS server running on https://localhost:${PORT}`);

  const logs: ConsoleEntry[] = [];

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
      viewport: { width: 800, height: 600 },
    });

    const page = await context.newPage();

    page.on("console", (msg) => {
      logs.push({ type: msg.type(), text: msg.text() });
    });

    page.on("pageerror", (err) => {
      logs.push({ type: "error", text: `[PAGE_ERROR] ${err.message}\n${err.stack}` });
    });

    // --- Step 1: Load page ---
    console.log("\n--- Step 1: Load page ---");
    await page.goto(`https://localhost:${PORT}/`, { waitUntil: "load" });

    const gpuInfo = await page.evaluate(async () => {
      if (!navigator.gpu) return { supported: false, reason: "navigator.gpu not available" };
      try {
        const adapter = await navigator.gpu.requestAdapter();
        if (!adapter) return { supported: false, reason: "no adapter returned" };
        return {
          supported: true,
          vendor: (adapter.info as any).vendor,
          architecture: (adapter.info as any).architecture,
        };
      } catch (e: any) {
        return { supported: false, reason: e.message };
      }
    });
    console.log("WebGPU:", JSON.stringify(gpuInfo));

    await page.waitForFunction(
      () => (window as any).turDemo !== undefined,
      { timeout: 10000 },
    );
    await waitForRender(page, 3000);

    let elements = await getLayout(page);
    await screenshot(page, "initial");

    // --- Step 2: Click "+ New Task" button ---
    console.log("\n--- Step 2: Click + New Task ---");
    let target = findClickTargetForText(elements, "+ New Task");
    if (click("+ New Task", target)) await page.mouse.click(target!.x, target!.y);
    await waitForRender(page);
    await screenshot(page, "click-new-task");

    // --- Step 3: Type in the modal's title input ---
    console.log("\n--- Step 3: Type in input ---");
    elements = await getLayout(page);
    const titleInput = findElement(elements, (e) => e.type === "tur_input" && e.rect.w > 300 && e.rect.h < 50);
    if (titleInput) {
      const c = center(titleInput.rect);
      console.log(`  Clicking title input at (${Math.round(c.x)}, ${Math.round(c.y)})`);
      await page.mouse.click(c.x, c.y);
      await waitForRender(page, 200);
      await page.keyboard.type("Buy groceries");
    } else {
      console.log("  WARNING: title input not found");
    }
    await waitForRender(page);
    await screenshot(page, "type-buy-groceries");

    // --- Step 4: Click "Add Task" button ---
    console.log("\n--- Step 4: Click Add Task ---");
    elements = await getLayout(page);
    target = findClickTargetForText(elements, "Add Task");
    if (click("Add Task", target)) await page.mouse.click(target!.x, target!.y);
    await waitForRender(page);
    await screenshot(page, "after-add-task");

    // --- Step 5: Add another task ---
    console.log("\n--- Step 5: Add another task ---");
    elements = await getLayout(page);
    target = findClickTargetForText(elements, "+ New Task");
    if (click("+ New Task", target)) await page.mouse.click(target!.x, target!.y);
    await waitForRender(page);

    elements = await getLayout(page);
    const input2 = findElement(elements, (e) => e.type === "tur_input" && e.rect.w > 300 && e.rect.h < 50);
    if (input2) {
      const c = center(input2.rect);
      await page.mouse.click(c.x, c.y);
      await waitForRender(page, 100);
      await page.keyboard.type("Review PR");
    }
    await waitForRender(page);

    elements = await getLayout(page);
    target = findClickTargetForText(elements, "Add Task");
    if (click("Add Task", target)) await page.mouse.click(target!.x, target!.y);
    await waitForRender(page);
    await screenshot(page, "after-add-second-task");

    // --- Step 6: Toggle checkbox on "Build tur engine" ---
    console.log("\n--- Step 6: Toggle checkbox ---");
    elements = await getLayout(page);
    target = findCheckboxForItem(elements, "Build tur engine");
    if (click("checkbox for Build tur engine", target)) await page.mouse.click(target!.x, target!.y);
    await waitForRender(page);
    await screenshot(page, "toggle-checkbox");

    // --- Step 7: Delete "Write documentation" ---
    console.log("\n--- Step 7: Delete a task ---");
    elements = await getLayout(page);
    target = findDeleteButtonForItem(elements, "Write documentation");
    if (click("delete for Write documentation", target)) await page.mouse.click(target!.x, target!.y);
    await waitForRender(page);
    await screenshot(page, "after-delete");

    // --- Step 8: Click on "Learn Rust" to select it ---
    console.log("\n--- Step 8: Select a todo item ---");
    elements = await getLayout(page);
    target = findItemRow(elements, "Learn Rust");
    if (click("row for Learn Rust", target)) await page.mouse.click(target!.x, target!.y);
    await waitForRender(page);
    await screenshot(page, "select-item");

    // Print final state
    elements = await getLayout(page);
    const taskSpans = elements.filter(
      (e) => e.type === "tur_text_span" && e.label && e.label !== "v" && e.label !== "x",
    );
    console.log("\n=== Final Task List ===");
    for (const span of taskSpans) {
      console.log(`  - ${span.label}`);
    }
    console.log("=== End Tasks ===\n");

    const debugLayout = await page.evaluate(() => {
      const w = window as any;
      return w.turDemo ? w.turDemo.debugLayout() : "";
    });
    console.log("=== Final Element Tree ===");
    console.log(debugLayout);
    console.log("=== End Tree ===");

    printConsole(logs);

    const errorCount = logs.filter(
      (e) => e.type === "error" || e.type === "warning",
    ).length;
    if (errorCount > 0) {
      console.log(`\n${errorCount} warnings/errors found in console`);
    }

    await browser.close();
  } finally {
    server.close();
  }
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
