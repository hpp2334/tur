import fs from "node:fs";
import https from "node:https";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { generate as generateCert } from "selfsigned";
import { chromium, type Page } from "playwright";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const DIST_DIR = path.resolve(__dirname, "../../tur-react-demo/dist");
const RESULTS_DIR = path.resolve(__dirname, "../test-results");
const PORT = 3999;

async function createServer(): Promise<https.Server> {
  const attrs = [{ name: "commonName", value: "localhost" }];
  const { cert, private: key } = await generateCert(attrs, {
    days: 365,
    algorithm: "sha256" as const,
    extensions: [{ name: "subjectAltName", altNames: [{ type: 2, value: "localhost" }] }],
  });
  return new Promise((resolve, reject) => {
    const MIME_TYPES: Record<string, string> = {
      ".html": "text/html", ".js": "text/javascript", ".wasm": "application/wasm",
      ".bin": "text/plain", ".map": "application/json",
    };
    const server = https.createServer({ cert, key }, (req, res) => {
      const urlPath = req.url === "/" ? "/index.html" : req.url!;
      const filePath = path.join(DIST_DIR, urlPath);
      if (!filePath.startsWith(DIST_DIR)) { res.writeHead(403); res.end(); return; }
      if (!fs.existsSync(filePath)) { res.writeHead(404); res.end(); return; }
      const ext = path.extname(filePath);
      res.writeHead(200, { "Content-Type": MIME_TYPES[ext] || "application/octet-stream", "Content-Length": fs.readFileSync(filePath).length });
      res.end(fs.readFileSync(filePath));
    });
    server.listen(PORT, () => resolve(server));
    server.on("error", reject);
  });
}

interface Rect { x: number; y: number; w: number; h: number }
interface ElementInfo { type: string; label: string; raw: string; rect: Rect }

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
    elements.push({ type, label, raw: line, rect: { x: parseFloat(x), y: parseFloat(y), w: parseFloat(w), h: parseFloat(h) } });
  }
  return elements;
}

function findElement(elements: ElementInfo[], predicate: (e: ElementInfo) => boolean): ElementInfo | undefined {
  return elements.find(predicate);
}
function findAll(elements: ElementInfo[], predicate: (e: ElementInfo) => boolean): ElementInfo[] {
  return elements.filter(predicate);
}
function center(r: Rect) { return { x: r.x + r.w / 2, y: r.y + r.h / 2 }; }
function contains(outer: Rect, inner: Rect) {
  return outer.x <= inner.x && outer.y <= inner.y && outer.x + outer.w >= inner.x + inner.w && outer.y + outer.h >= inner.y + inner.h;
}
function findSmallestContaining(elements: ElementInfo[], inner: Rect, type: string) {
  const candidates = findAll(elements, e => e.type === type && contains(e.rect, inner));
  candidates.sort((a, b) => (a.rect.w * a.rect.h) - (b.rect.w * b.rect.h));
  return candidates[0];
}

async function getLayout(page: Page): Promise<ElementInfo[]> {
  const layout: string | undefined = await page.evaluate(() => {
    const w = window as any;
    if (!w.turDemo) return "";
    const result = w.turDemo.debugLayout();
    return typeof result === "string" ? result : JSON.stringify(result);
  });
  const elements = parseLayout(layout || "");
  return elements;
}

async function samplePixels(page: Page, coords: { x: number; y: number }[]): Promise<{ r: number; g: number; b: number }[]> {
  // Take a full-page screenshot and sample pixels at the given coordinates
  const buf = await page.screenshot({ fullPage: true });
  const { PNG } = await import("pngjs");
  const png = PNG.sync.read(buf);
  return coords.map(p => {
    const idx = (Math.round(p.y) * png.width + Math.round(p.x)) * 4;
    return { r: png.data[idx], g: png.data[idx + 1], b: png.data[idx + 2] };
  });
}

function colorStr(c: { r: number; g: number; b: number }) {
  return `rgb(${c.r},${c.g},${c.b})`;
}

function isSimilarColor(a: { r: number; g: number; b: number }, b: { r: number; g: number; b: number }, threshold = 30) {
  return Math.abs(a.r - b.r) < threshold && Math.abs(a.g - b.g) < threshold && Math.abs(a.b - b.b) < threshold;
}

async function main() {
  fs.mkdirSync(RESULTS_DIR, { recursive: true });

  const browser = await chromium.launch({
    headless: true,
    args: ["--enable-unsafe-webgpu", "--use-angle=metal", "--ignore-gpu-blocklist", "--disable-software-rasterizer"],
  });

  console.log("Starting HTTPS server...");
  const server = await createServer();
  console.log(`HTTPS server on https://localhost:${PORT}`);

  const ctx = await browser.newContext({ ignoreHTTPSErrors: true, viewport: { width: 800, height: 600 } });
  const page = await ctx.newPage();

  console.log(`Visiting https://localhost:${PORT} ...`);
  await page.goto(`https://localhost:${PORT}/`, { waitUntil: "load", timeout: 30000 });

  const gpuInfo = await page.evaluate(async () => {
    if (!navigator.gpu) return { supported: false };
    try {
      const adapter = await navigator.gpu.requestAdapter();
      return { supported: !!adapter };
    } catch { return { supported: false }; }
  });
  console.log("WebGPU:", JSON.stringify(gpuInfo));

  if (!gpuInfo.supported) {
    console.log("SKIP: WebGPU not available");
    await browser.close();
    return;
  }

  await page.waitForFunction(() => (window as any).turDemo !== undefined, { timeout: 15000 });
  await page.waitForTimeout(3000);

  let passed = 0;
  let failed = 0;

  function check(label: string, condition: boolean, detail: string) {
    if (condition) {
      console.log(`  PASS: ${label}`);
      passed++;
    } else {
      console.log(`  FAIL: ${label} — ${detail}`);
      failed++;
    }
  }

  // ==================== INITIAL STATE ====================
  console.log("\n=== Initial State ===");
  let el = await getLayout(page);
  console.log(`  ${el.length} elements`);

  // Sample key pixels from initial render
  const initialSamples = await samplePixels(page, [
    { x: 10, y: 10 },       // top-left corner (should be sidebar dark)
    { x: 110, y: 300 },     // sidebar area (dark)
    { x: 400, y: 300 },     // main content area (light)
    { x: 500, y: 300 },     // task list area (white)
  ]);

  console.log("  Pixel samples (initial):");
  console.log(`    top-left (10,10):     ${colorStr(initialSamples[0])}`);
  console.log(`    sidebar (110,300):    ${colorStr(initialSamples[1])}`);
  console.log(`    main area (400,300):  ${colorStr(initialSamples[2])}`);
  console.log(`    task area (500,300):  ${colorStr(initialSamples[3])}`);

  // Sidebar should be dark (slate-900 ≈ rgb(15,23,42))
  check("sidebar is dark (#0f172a)", isSimilarColor(initialSamples[0], { r: 15, g: 23, b: 42 }), `got ${colorStr(initialSamples[0])}`);
  check("sidebar mid is dark", isSimilarColor(initialSamples[1], { r: 15, g: 23, b: 42 }), `got ${colorStr(initialSamples[1])}`);

  // Main content area should be light (#f8fafc)
  check("main area is light (#f8fafc)", isSimilarColor(initialSamples[2], { r: 248, g: 250, b: 252 }), `got ${colorStr(initialSamples[2])}`);

  // ==================== CHECK TASK CARDS ====================
  console.log("\n=== Task Cards ===");

  // Find task containers — they have border + white background
  const taskContainers = findAll(el, e => e.type === "tur_container" && e.raw.includes("borderColor=#e2e8f0") && e.rect.w > 500);
  console.log(`  Found ${taskContainers.length} task card containers`);

  check("4 task cards visible", taskContainers.length >= 4, `got ${taskContainers.length}`);

  // Sample a pixel from inside each task card — should be white
  if (taskContainers.length >= 4) {
    const cardPixels = await samplePixels(page, taskContainers.slice(0, 4).map(c => ({
      x: c.rect.x + c.rect.w / 2,
      y: c.rect.y + c.rect.h / 2,
    })));
    for (let i = 0; i < 4; i++) {
      const c = cardPixels[i];
      const isWhiteish = c.r > 240 && c.g > 240 && c.b > 240;
      check(`task card ${i + 1} is white`, isWhiteish, `got ${colorStr(c)}`);
    }
  }

  // ==================== CHECK SIDEBAR ACTIVE INDICATOR ====================
  console.log("\n=== Sidebar Active Indicator ===");

  // The "TodoList" item should have a purple (#6366f1) indicator bar on the left
  const activeIndicator = findElement(el, e =>
    e.type === "tur_container" &&
    e.raw.includes("color=#6366f1") &&
    e.rect.w <= 10 && e.rect.h > 20
  );
  if (activeIndicator) {
    const pixel = await samplePixels(page, [center(activeIndicator.rect)]);
    check("active indicator is purple (#6366f1)", isSimilarColor(pixel[0], { r: 99, g: 102, b: 241 }), `got ${colorStr(pixel[0])}`);
  } else {
    check("active indicator found", false, "no purple indicator element");
  }

  // ==================== CHECK "+ New Task" BUTTON ====================
  console.log("\n=== + New Task Button ===");

  const newTaskSpans = findAll(el, e => e.label === "+ New Task" && e.type === "tur_text_span");
  const newTaskBtn = newTaskSpans.length > 0
    ? findSmallestContaining(el, newTaskSpans[0].rect, "tur_pointer_interact")
    : undefined;
  if (newTaskBtn) {
    const pixel = await samplePixels(page, [center(newTaskBtn.rect)]);
    check("+ New Task button is purple (#6366f1)", isSimilarColor(pixel[0], { r: 99, g: 102, b: 241 }), `got ${colorStr(pixel[0])}`);
  }

  // ==================== CHECK CHECKBOXES ====================
  console.log("\n=== Checkboxes (initial - all unchecked) ===");

  // All initial checkboxes should be empty (no fill, just border)
  const uncheckedCheckboxes = findAll(el, e =>
    e.type === "tur_container" &&
    e.raw.includes("borderColor=#e2e8f0") &&
    e.rect.w <= 25 && e.rect.h <= 25 && e.rect.w >= 18
  );
  console.log(`  Found ${uncheckedCheckboxes.length} unchecked checkboxes (border only)`);

  if (uncheckedCheckboxes.length > 0) {
    const cb = uncheckedCheckboxes[0];
    const pixel = await samplePixels(page, [center(cb.rect)]);
    // Unchecked should have a light border and white or light fill
    const isLight = pixel[0].r > 230 && pixel[0].g > 230 && pixel[0].b > 230;
    check("unchecked checkbox is light/white", isLight, `got ${colorStr(pixel[0])}`);
  }

  // ==================== TOGGLE CHECKBOX ====================
  console.log("\n=== Toggle Checkbox ===");

  // Toggle "Build tur engine" checkbox (second task)
  const buildTurSpan = findAll(el, e => e.label === "Build tur engine" && e.type === "tur_text_span");
  if (buildTurSpan.length > 0) {
    const spanY = buildTurSpan[0].rect.y;
    const spanX = buildTurSpan[0].rect.x;
    
    const checkbox = findElement(el, e =>
      e.type === "tur_pointer_interact" &&
      e.rect.w <= 30 && e.rect.h <= 30 &&
      e.rect.x < spanX &&
      Math.abs(e.rect.y + e.rect.h / 2 - spanY) < 20
    );
    if (checkbox) {
      const c = center(checkbox.rect);
      console.log(`  Clicking checkbox at (${Math.round(c.x)}, ${Math.round(c.y)})`);
      await page.mouse.click(c.x, c.y, { delay: 50 });
      await page.waitForTimeout(1000);

      el = await getLayout(page);

      const checkedContainer = findElement(el, e =>
        e.type === "tur_container" &&
        e.raw.includes("color=#22c55e") &&
        e.rect.w <= 25 && e.rect.h <= 25
      );
      check("checked checkbox has green fill (#22c55e)", !!checkedContainer, "no green checkbox found");

      if (checkedContainer) {
        const r = checkedContainer.rect;
        const pixel = await samplePixels(page, [{ x: r.x + r.w - 4, y: r.y + r.h - 4 }]);
        check("checked checkbox pixel is green", isSimilarColor(pixel[0], { r: 34, g: 197, b: 94 }, 40), `got ${colorStr(pixel[0])}`);
      }

      // Check for checkmark text "v"
      const checkmark = findElement(el, e =>
        e.type === "tur_text_span" &&
        e.label === "v" &&
        Math.abs(e.rect.y - spanY) < 20
      );
      check("checkmark text 'v' present", !!checkmark, "no checkmark text found");
    }
  }

  // ==================== DELETE BUTTON COLOR ====================
  console.log("\n=== Delete Button ===");

  el = await getLayout(page);
  const deleteBtns = findAll(el, e =>
    e.type === "tur_container" &&
    e.raw.includes("color=#fef2f2") &&
    e.rect.w <= 35 && e.rect.h <= 35
  );
  console.log(`  Found ${deleteBtns.length} delete buttons (red-tinted bg)`);

  if (deleteBtns.length > 0) {
    const db = deleteBtns[0];
    const pixel = await samplePixels(page, [{ x: db.rect.x + 4, y: db.rect.y + 4 }]);
    check("delete button has red-tinted bg (#fef2f2)", isSimilarColor(pixel[0], { r: 254, g: 242, b: 242 }), `got ${colorStr(pixel[0])}`);
  }

  // ==================== FINAL SUMMARY ====================
  console.log(`\n=== Visual Checks: ${passed} passed, ${failed} failed ===`);

  await browser.close();
  server.close();

  if (failed > 0) process.exit(1);
}

main().catch(err => { console.error(err); process.exit(1); });
