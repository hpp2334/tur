import fs from "node:fs";
import https from "node:https";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { chromium, type Page } from "playwright";
import { generate as generateCert } from "selfsigned";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const DIST_DIR = path.resolve(__dirname, "../../tur-react-demo/dist");
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
        const absMatch = line.match(
            /abs\(([^,]+),([^)]+)\)\s+([\d.]+)x([\d.]+)/,
        );
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

function contains(outer: Rect, inner: Rect): boolean {
    return (
        outer.x <= inner.x &&
        outer.y <= inner.y &&
        outer.x + outer.w >= inner.x + inner.w &&
        outer.y + outer.h >= inner.y + inner.h
    );
}

function _rectCenterContainsPoint(r: Rect, px: number, py: number): boolean {
    return px >= r.x && px <= r.x + r.w && py >= r.y && py <= r.y + r.h;
}

function findAll(
    elements: ElementInfo[],
    predicate: (e: ElementInfo) => boolean,
): ElementInfo[] {
    return elements.filter(predicate);
}

function findElement(
    elements: ElementInfo[],
    predicate: (e: ElementInfo) => boolean,
): ElementInfo | undefined {
    return elements.find(predicate);
}

function findTextSpans(elements: ElementInfo[], text: string): ElementInfo[] {
    return findAll(
        elements,
        (e) =>
            e.label === text &&
            (e.type === "tur_text_span" || e.type === "tur_paragraph"),
    );
}

function findSmallestContaining(
    elements: ElementInfo[],
    innerRect: Rect,
    type: string,
): ElementInfo | undefined {
    const candidates = findAll(
        elements,
        (e) => e.type === type && contains(e.rect, innerRect),
    );
    candidates.sort((a, b) => a.rect.w * a.rect.h - b.rect.w * b.rect.h);
    return candidates[0];
}

function findModalAddTaskButton(
    elements: ElementInfo[],
): { x: number; y: number } | null {
    const spans = findTextSpans(elements, "Add Task");
    for (const span of spans) {
        const btn = findSmallestContaining(
            elements,
            span.rect,
            "tur_pointer_interact",
        );
        if (btn) {
            return center(btn.rect);
        }
    }
    return null;
}

function findHeaderNewTaskButton(
    elements: ElementInfo[],
): { x: number; y: number } | null {
    const spans = findTextSpans(elements, "+ New Task");
    for (const span of spans) {
        const btn = findSmallestContaining(
            elements,
            span.rect,
            "tur_pointer_interact",
        );
        if (btn) {
            return center(btn.rect);
        }
    }
    return null;
}

function findModalInput(
    elements: ElementInfo[],
    index: number = 0,
): ElementInfo | undefined {
    const inputs = findAll(
        elements,
        (e) =>
            (e.type === "tur_input" || e.type === "tur_editable_text") &&
            e.rect.h < 50,
    );
    return inputs[index];
}

function _findTaskListInput(elements: ElementInfo[]): ElementInfo | undefined {
    const inputs = findAll(
        elements,
        (e) => e.type === "tur_input" && e.rect.w > 500,
    );
    return inputs.length > 0 ? inputs[0] : undefined;
}

function findCheckboxForItem(
    elements: ElementInfo[],
    itemText: string,
): { x: number; y: number } | null {
    const spans = findTextSpans(elements, itemText);
    for (const span of spans) {
        const sy = span.rect.y;
        const checkbox = findElement(
            elements,
            (e) =>
                e.type === "tur_pointer_interact" &&
                e.rect.w <= 30 &&
                e.rect.h <= 30 &&
                Math.abs(e.rect.y + e.rect.h / 2 - (sy + span.rect.h / 2)) <
                    20 &&
                e.rect.x < span.rect.x,
        );
        if (checkbox) return center(checkbox.rect);
    }
    return null;
}

function findDeleteButtonForItem(
    elements: ElementInfo[],
    itemText: string,
): { x: number; y: number } | null {
    const spans = findTextSpans(elements, itemText);
    for (const span of spans) {
        const sy = span.rect.y;
        const delBtn = findElement(
            elements,
            (e) =>
                e.type === "tur_pointer_interact" &&
                e.rect.w <= 40 &&
                e.rect.h <= 40 &&
                Math.abs(e.rect.y + e.rect.h / 2 - (sy + span.rect.h / 2)) <
                    20 &&
                e.rect.x > span.rect.x + 100,
        );
        if (delBtn) return center(delBtn.rect);
    }
    return null;
}

function findItemRow(
    elements: ElementInfo[],
    itemText: string,
): { x: number; y: number } | null {
    const spans = findTextSpans(elements, itemText);
    for (const span of spans) {
        const row = findSmallestContaining(
            elements,
            span.rect,
            "tur_pointer_interact",
        );
        if (row && row.rect.w > 100) return center(row.rect);
    }
    return null;
}

function hasModalOpen(elements: ElementInfo[]): boolean {
    return elements.some(
        (e) =>
            e.label === "Add New Task" &&
            (e.type === "tur_text_span" || e.type === "tur_paragraph"),
    );
}

function getTaskTexts(elements: ElementInfo[]): string[] {
    const taskItems: string[] = [];
    for (const e of elements) {
        if (
            (e.type === "tur_text_span" || e.type === "tur_paragraph") &&
            e.label &&
            e.label !== "v" &&
            e.label !== "x" &&
            e.label !== "My Tasks" &&
            e.label !== "NAVIGATION" &&
            e.label !== "Tur Todo" &&
            e.label !== "TodoList" &&
            e.label !== "+ New Task" &&
            e.label !== "Add New Task" &&
            e.label !== "Add Task" &&
            e.label !== "Cancel" &&
            e.label !== "Title" &&
            e.label !== "Description" &&
            e.rect.h > 5
        ) {
            const hasDeleteBtn = elements.some(
                (d) =>
                    d.type === "tur_pointer_interact" &&
                    d.rect.w <= 40 &&
                    d.rect.h <= 40 &&
                    d.rect.x > e.rect.x + e.rect.w &&
                    Math.abs(d.rect.y - e.rect.y) < 30,
            );
            const hasCheckbox = elements.some(
                (c) =>
                    c.type === "tur_pointer_interact" &&
                    c.rect.w <= 30 &&
                    c.rect.h <= 30 &&
                    c.rect.x < e.rect.x &&
                    Math.abs(c.rect.y - e.rect.y) < 25,
            );
            if (hasDeleteBtn && hasCheckbox) {
                taskItems.push(e.label);
            }
        }
    }
    return taskItems;
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
        const w = window as Record<string, unknown>;
        if (!w.turDemo) return "";
        const result = w.turDemo.debugLayout();
        return typeof result === "string" ? result : JSON.stringify(result);
    });
    const elements = parseLayout(layout || "");
    console.log(
        `  Parsed ${elements.length} elements (${(layout || "").length} chars)`,
    );
    return elements;
}

async function assertClick(
    page: Page,
    _elements: ElementInfo[],
    label: string,
    target: { x: number; y: number } | null,
    offset: { x: number; y: number } = { x: 0, y: 0 },
) {
    if (!target) {
        throw new Error(`ASSERT FAIL: could not find "${label}" to click`);
    }
    const px = Math.round(target.x + offset.x);
    const py = Math.round(target.y + offset.y);
    console.log(`  Clicking "${label}" at (${px}, ${py})`);
    await page.mouse.click(px, py);
    await waitForRender(page);
}

async function printConsole(logs: ConsoleEntry[]) {
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
    let passed = 0;
    let failed = 0;

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
            permissions: ["clipboard-read", "clipboard-write"],
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

        // --- Step 1: Load page ---
        console.log("\n--- Step 1: Load page ---");
        await page.goto(`https://localhost:${PORT}/`, { waitUntil: "load" });

        const gpuInfo = await page.evaluate(async () => {
            if (!navigator.gpu)
                return {
                    supported: false,
                    reason: "navigator.gpu not available",
                };
            try {
                const adapter = await navigator.gpu.requestAdapter();
                if (!adapter)
                    return { supported: false, reason: "no adapter returned" };
                return {
                    supported: true,
                    vendor: (adapter.info as Record<string, string>).vendor,
                    architecture: (adapter.info as Record<string, string>)
                        .architecture,
                };
            } catch (e: unknown) {
                const msg = e instanceof Error ? e.message : String(e);
                return { supported: false, reason: msg };
            }
        });
        console.log("WebGPU:", JSON.stringify(gpuInfo));

        await waitForRender(page, 1000);

        // --- Step 2: Click "todolist" in sidebar ---
        console.log("\n--- Step 2: Select todolist case ---");
        const todolistBtn = page.locator("button.case-item", {
            hasText: /^todolist$/,
        });
        await todolistBtn.click();

        await page.waitForFunction(
            () => {
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
            { timeout: 30000 },
        );
        await waitForRender(page, 1000);

        const canvasOffset = await page.evaluate(() => {
            const canvas = document.querySelector("#tur-container canvas");
            if (!canvas) return { x: 0, y: 0 };
            const rect = canvas.getBoundingClientRect();
            return { x: rect.x, y: rect.y };
        });
        console.log(`  Canvas offset: ${JSON.stringify(canvasOffset)}`);

        let elements = await getLayout(page);
        await screenshot(page, "initial");

        // VERIFY: initial state - 4 tasks, no modal
        const initialModal = hasModalOpen(elements);
        console.log(`  Initial modal open: ${initialModal}`);
        if (initialModal) {
            console.log("  FAIL: modal should not be open initially");
            failed++;
        } else {
            console.log("  PASS: no modal on initial load");
            passed++;
        }

        const initialTasks = getTaskTexts(elements);
        console.log(`  Initial tasks: ${initialTasks.join(", ")}`);
        if (initialTasks.length >= 4) {
            console.log("  PASS: initial tasks present");
            passed++;
        } else {
            console.log(
                `  FAIL: expected >=4 tasks, got ${initialTasks.length}`,
            );
            failed++;
        }

        // --- Step 3: Click "+ New Task" button ---
        console.log("\n--- Step 3: Click + New Task ---");
        elements = await getLayout(page);
        const newTaskBtn = findHeaderNewTaskButton(elements);
        await assertClick(
            page,
            elements,
            "+ New Task (header)",
            newTaskBtn,
            canvasOffset,
        );
        await screenshot(page, "click-new-task");

        // VERIFY: modal should be open
        elements = await getLayout(page);
        if (hasModalOpen(elements)) {
            console.log("  PASS: modal opened after clicking + New Task");
            passed++;
        } else {
            console.log("  FAIL: modal did not open");
            failed++;
        }

        // VERIFY: modal has input fields
        const modalInput = findModalInput(elements, 0);
        if (modalInput) {
            console.log(
                `  PASS: found modal title input at (${modalInput.rect.x}, ${modalInput.rect.y}) ${modalInput.rect.w}x${modalInput.rect.h}`,
            );
            passed++;
        } else {
            console.log("  FAIL: modal title input not found");
            failed++;
        }

        // --- Step 4: Type in the modal's title input ---
        console.log("\n--- Step 4: Type in input ---");
        elements = await getLayout(page);
        const titleInput = findModalInput(elements, 0);
        if (titleInput && canvasOffset) {
            const c = center(titleInput.rect);
            const px = Math.round(c.x + canvasOffset.x);
            const py = Math.round(c.y + canvasOffset.y);
            console.log(`  Clicking title input at (${px}, ${py})`);
            await page.mouse.click(px, py);
            await waitForRender(page, 200);
            await page.keyboard.type("Buy groceries");
        }
        await waitForRender(page);
        await screenshot(page, "type-buy-groceries");

        // VERIFY: input has text
        elements = await getLayout(page);
        const typedInput = findElement(
            elements,
            (e) =>
                (e.type === "tur_input" || e.type === "tur_editable_text") &&
                e.label.includes("Buy groceries"),
        );
        if (typedInput) {
            console.log(`  PASS: input shows "${typedInput.label}"`);
            passed++;
        } else {
            const allInputs = findAll(elements, (e) => e.type === "tur_input");
            const allTexts = findAll(
                elements,
                (e) =>
                    (e.type === "tur_text_span" ||
                        e.type === "tur_paragraph") &&
                    e.label.includes("Buy groceries"),
            );
            console.log(
                `  FAIL: input does not show "Buy groceries". Inputs: ${allInputs.map((e) => `"${e.label}"`).join(", ")}. Texts: ${allTexts.map((e) => `"${e.label}"`).join(", ")}`,
            );
            failed++;
        }

        // --- Step 5: Click "Add Task" button (in modal) ---
        console.log("\n--- Step 5: Click Add Task (modal) ---");
        elements = await getLayout(page);
        const addTaskBtn = findModalAddTaskButton(elements);
        await assertClick(
            page,
            elements,
            "Add Task (modal)",
            addTaskBtn,
            canvasOffset,
        );
        await screenshot(page, "after-add-task");

        // VERIFY: modal should be closed
        elements = await getLayout(page);
        if (!hasModalOpen(elements)) {
            console.log("  PASS: modal closed after adding task");
            passed++;
        } else {
            console.log("  FAIL: modal still open after clicking Add Task");
            failed++;
        }

        // VERIFY: new task appears in the list
        const taskTextsAfterAdd = getTaskTexts(elements);
        console.log(`  Tasks after add: ${taskTextsAfterAdd.join(", ")}`);
        if (taskTextsAfterAdd.some((t) => t.includes("Buy groceries"))) {
            console.log("  PASS: 'Buy groceries' appears in task list");
            passed++;
        } else {
            console.log("  FAIL: 'Buy groceries' not found in task list");
            failed++;
        }

        // --- Step 6: Add another task ---
        console.log("\n--- Step 6: Add second task 'Review PR' ---");
        elements = await getLayout(page);
        const newTaskBtn2 = findHeaderNewTaskButton(elements);
        await assertClick(
            page,
            elements,
            "+ New Task (header)",
            newTaskBtn2,
            canvasOffset,
        );
        await screenshot(page, "open-modal-again");

        elements = await getLayout(page);
        if (!hasModalOpen(elements)) {
            console.log("  FAIL: modal did not open for second task");
            failed++;
        } else {
            const input2 = findModalInput(elements, 0);
            if (input2 && canvasOffset) {
                const c = center(input2.rect);
                const px = Math.round(c.x + canvasOffset.x);
                const py = Math.round(c.y + canvasOffset.y);
                console.log(`  Clicking title input at (${px}, ${py})`);
                await page.mouse.click(px, py);
                await waitForRender(page, 200);
                await page.keyboard.type("Review PR");
            }
            await waitForRender(page);

            elements = await getLayout(page);
            const addTaskBtn2 = findModalAddTaskButton(elements);
            await assertClick(
                page,
                elements,
                "Add Task (modal)",
                addTaskBtn2,
                canvasOffset,
            );
        }
        await screenshot(page, "after-add-second-task");

        // VERIFY: second task added, modal closed
        elements = await getLayout(page);
        const taskTextsAfterAdd2 = getTaskTexts(elements);
        console.log(
            `  Tasks after second add: ${taskTextsAfterAdd2.join(", ")}`,
        );
        if (!hasModalOpen(elements)) {
            console.log("  PASS: modal closed after adding second task");
            passed++;
        } else {
            console.log("  FAIL: modal still open after adding second task");
            failed++;
        }
        if (taskTextsAfterAdd2.some((t) => t.includes("Review PR"))) {
            console.log("  PASS: 'Review PR' appears in task list");
            passed++;
        } else {
            console.log("  FAIL: 'Review PR' not found in task list");
            failed++;
        }

        // --- Step 7: Toggle checkbox on "Build tur engine" ---
        console.log("\n--- Step 7: Toggle checkbox ---");
        elements = await getLayout(page);
        const checkbox = findCheckboxForItem(elements, "Build tur engine");
        await assertClick(
            page,
            elements,
            "checkbox for Build tur engine",
            checkbox,
            canvasOffset,
        );
        await screenshot(page, "toggle-checkbox");

        // VERIFY: checkbox is now checked (has "v" checkmark text inside)
        elements = await getLayout(page);
        const buildTurSpan = findTextSpans(elements, "Build tur engine");
        if (buildTurSpan.length > 0) {
            const sy = buildTurSpan[0].rect.y;
            const checkmark = findElement(
                elements,
                (e) =>
                    (e.type === "tur_text_span" ||
                        e.type === "tur_paragraph") &&
                    e.label === "v" &&
                    Math.abs(
                        e.rect.y +
                            e.rect.h / 2 -
                            (sy + buildTurSpan[0].rect.h / 2),
                    ) < 20 &&
                    e.rect.x < buildTurSpan[0].rect.x,
            );
            if (checkmark) {
                console.log('  PASS: checkbox has checkmark ("v")');
                passed++;
            } else {
                console.log("  FAIL: checkbox checkmark not found");
                failed++;
            }
        }

        // --- Step 8: Delete "Write documentation" ---
        console.log("\n--- Step 8: Delete 'Write documentation' ---");
        elements = await getLayout(page);
        const deleteBtn = findDeleteButtonForItem(
            elements,
            "Write documentation",
        );
        await assertClick(
            page,
            elements,
            "delete for Write documentation",
            deleteBtn,
            canvasOffset,
        );
        await screenshot(page, "after-delete");

        // VERIFY: "Write documentation" is gone
        elements = await getLayout(page);
        const taskTextsAfterDelete = getTaskTexts(elements);
        console.log(`  Tasks after delete: ${taskTextsAfterDelete.join(", ")}`);
        if (!taskTextsAfterDelete.includes("Write documentation")) {
            console.log("  PASS: 'Write documentation' removed from task list");
            passed++;
        } else {
            console.log(
                "  FAIL: 'Write documentation' still present after delete",
            );
            failed++;
        }

        // --- Step 9: Click on "Learn Rust" to select it ---
        console.log("\n--- Step 9: Select 'Learn Rust' ---");
        elements = await getLayout(page);
        const learnRustRow = findItemRow(elements, "Learn Rust");
        await assertClick(
            page,
            elements,
            "row for Learn Rust",
            learnRustRow,
            canvasOffset,
        );
        await screenshot(page, "select-item");

        // Print final state
        elements = await getLayout(page);
        const finalTasks = getTaskTexts(elements);
        console.log("\n=== Final Task List ===");
        for (const task of finalTasks) {
            console.log(`  - ${task}`);
        }
        console.log("=== End Tasks ===\n");

        // VERIFY: modal not open at end
        if (!hasModalOpen(elements)) {
            console.log("  PASS: no modal open at end");
            passed++;
        } else {
            console.log("  FAIL: modal is still open at end");
            failed++;
        }

        // VERIFY: correct tasks present
        const expectedTasks = [
            "Learn Rust",
            "Build tur engine",
            "Ship v0.1.0",
            "Buy groceries",
            "Review PR",
        ];
        const allExpectedPresent = expectedTasks.every((t) =>
            finalTasks.includes(t),
        );
        const writeDocGone = !finalTasks.includes("Write documentation");
        if (allExpectedPresent && writeDocGone) {
            console.log(
                `  PASS: correct tasks present (${finalTasks.length} items including detail panel texts)`,
            );
            passed++;
        } else {
            const missing = expectedTasks.filter(
                (t) => !finalTasks.includes(t),
            );
            const unexpected = finalTasks.filter(
                (t) =>
                    !expectedTasks.includes(t) && t !== "Write documentation",
            );
            console.log(
                `  FAIL: missing=${missing.join(",")} unexpected=${unexpected.join(",")}`,
            );
            failed++;
        }

        printConsole(logs);

        // ========== FILE TREE TEST ==========
        console.log("\n========== FILE TREE TEST ==========");

        // --- Step F1: Verify file tree has todolist expanded with files ---
        console.log("\n--- Step F1: Verify file tree for todolist ---");
        const fileTreeVisible = await page.evaluate(() => {
            const items = document.querySelectorAll(".file-item");
            return Array.from(items).map((el) => el.textContent);
        });
        console.log(`  File tree items: ${fileTreeVisible.join(", ")}`);
        const expectedFiles = [
            "Sidebar.tsx",
            "index.tsx",
            "store.ts",
            "theme.ts",
        ];
        const allFilesPresent = expectedFiles.every((f) =>
            fileTreeVisible.includes(f),
        );
        if (allFilesPresent) {
            console.log("  PASS: all todolist files visible in file tree");
            passed++;
        } else {
            console.log(
                `  FAIL: expected ${expectedFiles.join(", ")}, got ${fileTreeVisible.join(", ")}`,
            );
            failed++;
        }

        const activeFile = await page.evaluate(() => {
            const active = document.querySelector(".file-item.active");
            return active ? active.textContent : null;
        });
        if (activeFile === "index.tsx") {
            console.log("  PASS: index.tsx is the active file");
            passed++;
        } else {
            console.log(`  FAIL: expected index.tsx active, got ${activeFile}`);
            failed++;
        }

        // --- Step F2: Click store.ts in file tree ---
        console.log("\n--- Step F2: Click store.ts in file tree ---");
        const storeFileBtn = page.locator("button.file-item", {
            hasText: /^store\.ts$/,
        });
        await storeFileBtn.click();
        await waitForRender(page, 200);

        const editorContent = await page.evaluate(() => {
            const lines = document.querySelectorAll(
                ".cm-editor .cm-content .cm-line",
            );
            return Array.from(lines)
                .slice(0, 3)
                .map((l) => l.textContent)
                .join("\n");
        });
        console.log(`  Editor first 3 lines: ${editorContent}`);
        if (
            editorContent.includes("createTextEditingController") ||
            editorContent.includes("jotai")
        ) {
            console.log("  PASS: editor shows store.ts content");
            passed++;
        } else {
            console.log("  FAIL: editor does not show store.ts content");
            failed++;
        }

        const activeFileAfterSwitch = await page.evaluate(() => {
            const active = document.querySelector(".file-item.active");
            return active ? active.textContent : null;
        });
        if (activeFileAfterSwitch === "store.ts") {
            console.log("  PASS: store.ts is now the active file");
            passed++;
        } else {
            console.log(
                `  FAIL: expected store.ts active, got ${activeFileAfterSwitch}`,
            );
            failed++;
        }

        await screenshot(page, "file-tree-store");

        // --- Step F3: Switch back to index.tsx ---
        console.log("\n--- Step F3: Click index.tsx in file tree ---");
        const indexFileBtn = page.locator("button.file-item", {
            hasText: /^index\.tsx$/,
        });
        await indexFileBtn.click();
        await waitForRender(page, 200);

        const editorContentIndex = await page.evaluate(() => {
            const lines = document.querySelectorAll(
                ".cm-editor .cm-content .cm-line",
            );
            return Array.from(lines)
                .slice(0, 3)
                .map((l) => l.textContent)
                .join("\n");
        });
        if (
            editorContentIndex.includes("@tur/react") ||
            editorContentIndex.includes("import")
        ) {
            console.log("  PASS: editor shows index.tsx content");
            passed++;
        } else {
            console.log("  FAIL: editor does not show index.tsx content");
            failed++;
        }

        // --- Step F4: Verify canvas still renders correctly after file switching ---
        console.log(
            "\n--- Step F4: Verify canvas still renders after file switching ---",
        );
        await screenshot(page, "file-tree-after-switch");
        const elementsAfterSwitch = await getLayout(page);
        const tasksAfterSwitch = getTaskTexts(elementsAfterSwitch);
        if (tasksAfterSwitch.length >= 4) {
            console.log(
                "  PASS: canvas still renders tasks after file switching",
            );
            passed++;
        } else {
            console.log(
                `  FAIL: canvas lost tasks after file switching (got ${tasksAfterSwitch.length})`,
            );
            failed++;
        }

        // --- Step F5: Switch to counter case (single file, no tree) ---
        console.log("\n--- Step F5: Switch to counter case ---");
        const counterBtn = page.locator("button.case-item", {
            hasText: /^counter$/,
        });
        await counterBtn.click();
        await waitForRender(page, 500);

        const counterFileTree = await page.evaluate(() => {
            const items = document.querySelectorAll(".file-item");
            return Array.from(items).map((el) => el.textContent);
        });
        console.log(`  Counter file tree items: ${counterFileTree.join(", ")}`);
        if (counterFileTree.length === 0) {
            console.log("  PASS: no file tree for single-file counter case");
            passed++;
        } else {
            console.log(
                `  FAIL: unexpected file tree items for counter: ${counterFileTree.join(", ")}`,
            );
            failed++;
        }

        const editorHeader = await page.evaluate(() => {
            const header = document.querySelector(".editor-header span");
            return header ? header.textContent : null;
        });
        if (editorHeader?.includes("counter/index.tsx")) {
            console.log("  PASS: editor header shows counter/index.tsx");
            passed++;
        } else {
            console.log(
                `  FAIL: expected counter/index.tsx header, got ${editorHeader}`,
            );
            failed++;
        }

        await screenshot(page, "file-tree-counter");

        // ========== COUNTER APP TEST ==========
        // Test live editing: type a counter app in the code editor, compile, and interact
        console.log("\n========== COUNTER APP TEST ==========");

        const counterSource = `import { Column, Container, Expanded, PointerInteract, Row, SizedBox, Text, Color, MainAxisAlignment, CrossAxisAlignment } from "@tur/react";
import { renderRoot } from "@tur/react-renderer";
import { useState } from "react";

function Counter() {
  const [count, setCount] = useState(0);
  return (
    <Expanded>
      <Container color={Color.hex("#f8fafc")}>
        <Column mainAlignment={MainAxisAlignment.Center} crossAlignment={CrossAxisAlignment.Center}>
          <Container width={300} borderRadius={12} padding={24} color={Color.hex("#ffffff")}>
            <Column crossAlignment={CrossAxisAlignment.Center}>
              <Text content={"Count: " + count} queryKey={["count"]} fontSize={36} color={Color.hex("#1e293b")} />
              <SizedBox height={20} />
              <Row mainAlignment={MainAxisAlignment.Center}>
                <PointerInteract
                  onClick={() => setCount((n) => n + 1)}
                  child={<Container width={100} height={44} borderRadius={8} color={Color.hex("#6366f1")}><Row mainAlignment={MainAxisAlignment.Center} crossAlignment={CrossAxisAlignment.Center}><Text content="+1" fontSize={18} color={Color.hex("#ffffff")} /></Row></Container>}
                />
                <SizedBox width={12} />
                <PointerInteract
                  onClick={() => setCount((n) => n - 1)}
                  child={<Container width={100} height={44} borderRadius={8} color={Color.hex("#ef4444")}><Row mainAlignment={MainAxisAlignment.Center} crossAlignment={CrossAxisAlignment.Center}><Text content="-1" fontSize={18} color={Color.hex("#ffffff")} /></Row></Container>}
                />
              </Row>
            </Column>
          </Container>
        </Column>
      </Container>
    </Expanded>
  );
}

renderRoot(Counter);
`;

        // --- Step A: Select "todolist" case to open the editor ---
        console.log("\n--- Step A: Select todolist case (opens editor) ---");
        const editorCaseBtn = page.locator("button.case-item", {
            hasText: /^todolist$/,
        });
        await editorCaseBtn.click();
        await waitForRender(page, 500);

        // Wait for the code editor to load with source
        await page.waitForFunction(
            () => {
                const editor = document.querySelector(".cm-editor .cm-content");
                return editor?.textContent && editor.textContent.length > 10;
            },
            { timeout: 5000 },
        );

        // --- Step B: Clear editor and paste the counter source ---
        console.log("\n--- Step B: Paste counter app in editor ---");
        const cmContent = page.locator(".cm-editor .cm-content");
        await cmContent.click();
        await page.keyboard.press("Meta+a");
        await waitForRender(page, 100);

        await page.evaluate(async (code) => {
            await navigator.clipboard.writeText(code);
        }, counterSource);
        await waitForRender(page, 100);
        await page.keyboard.press("Meta+v");
        await waitForRender(page, 300);

        const editorText = await page.evaluate(() => {
            const lines = document.querySelectorAll(
                ".cm-editor .cm-content .cm-line",
            );
            return Array.from(lines)
                .map((l) => l.textContent)
                .join("\n");
        });
        console.log(
            `  Editor has "Row": ${editorText.includes("Row")}, length: ${editorText.length}`,
        );
        if (!editorText.includes("Row")) {
            console.log(
                `  Editor first 80 chars: "${editorText.substring(0, 80)}"`,
            );
        }

        // --- Step C: Save (Cmd+S) to compile ---
        console.log("\n--- Step C: Save (Cmd+S) to compile ---");
        await page.keyboard.press("Meta+s");
        await waitForRender(page, 500);

        await page.waitForFunction(
            () => {
                const w = window as Record<string, unknown>;
                if (!w.turDemo) return false;
                try {
                    const layout = (
                        w.turDemo as { debugLayout: () => string }
                    ).debugLayout();
                    return (
                        typeof layout === "string" && layout.includes("Count:")
                    );
                } catch {
                    return false;
                }
            },
            { timeout: 15000 },
        );
        await waitForRender(page, 500);

        const buildErrorText = await page.evaluate(() => {
            const errEl = document.querySelector(".build-error");
            return errEl?.textContent ?? null;
        });
        if (buildErrorText) {
            const buildErrorTitle = await page.evaluate(() => {
                const errEl = document.querySelector(".build-error");
                return errEl?.getAttribute("title") ?? "";
            });
            console.log(`  BUILD ERROR: ${buildErrorTitle}`);
        } else {
            console.log("  No build error badge");
        }

        await screenshot(page, "counter-initial");

        elements = await getLayout(page);

        console.log("  All elements:");
        for (const el of elements) {
            console.log(
                `    ${el.type} "${el.label}" at (${el.rect.x},${el.rect.y}) ${el.rect.w}x${el.rect.h}`,
            );
        }

        const allTexts = findAll(
            elements,
            (e) => e.type === "tur_text_span" || e.type === "tur_paragraph",
        );
        console.log(
            "  All text labels:",
            allTexts.map((e) => `"${e.label}"`).join(", "),
        );

        const counterErrors = logs.filter((e) => e.type === "error");
        if (counterErrors.length > 0) {
            console.log("  Browser errors:");
            for (const err of counterErrors) {
                console.log(`    ${err.text}`);
            }
        }

        const countZero = findTextSpans(elements, "Count: 0");
        if (countZero.length > 0) {
            console.log('  PASS: "Count: 0" rendered');
            passed++;
        } else {
            console.log('  FAIL: "Count: 0" not found');
            failed++;
        }

        // VERIFY: "+1" button exists
        const plusOneTexts = findTextSpans(elements, "+1");
        if (plusOneTexts.length > 0) {
            console.log('  PASS: "+1" button text found');
            passed++;
        } else {
            console.log('  FAIL: "+1" button text not found');
            failed++;
        }

        // VERIFY: "-1" button exists
        const minusOneTexts = findTextSpans(elements, "-1");
        if (minusOneTexts.length > 0) {
            console.log('  PASS: "-1" button text found');
            passed++;
        } else {
            console.log('  FAIL: "-1" button text not found');
            failed++;
        }

        // --- Step D: Click "+1" button ---
        console.log("\n--- Step D: Click +1 ---");
        elements = await getLayout(page);
        const plusOneSpan = findTextSpans(elements, "+1");
        let plusOneClickTarget: { x: number; y: number } | null = null;
        if (plusOneSpan.length > 0) {
            const btn = findSmallestContaining(
                elements,
                plusOneSpan[0].rect,
                "tur_pointer_interact",
            );
            if (btn) {
                plusOneClickTarget = center(btn.rect);
            }
        }
        await assertClick(
            page,
            elements,
            "+1 button",
            plusOneClickTarget,
            canvasOffset,
        );
        await screenshot(page, "counter-click-plus1");

        // VERIFY: count is now 1
        elements = await getLayout(page);
        const countOne = findTextSpans(elements, "Count: 1");
        if (countOne.length > 0) {
            console.log('  PASS: "Count: 1" after clicking +1');
            passed++;
        } else {
            console.log('  FAIL: "Count: 1" not found after clicking +1');
            const allTexts = findAll(
                elements,
                (e) => e.type === "tur_text_span" || e.type === "tur_paragraph",
            );
            console.log(
                "  Texts:",
                allTexts.map((e) => `"${e.label}"`).join(", "),
            );
            failed++;
        }

        // --- Step E: Click "+1" again ---
        console.log("\n--- Step E: Click +1 again ---");
        elements = await getLayout(page);
        const plusOneSpan2 = findTextSpans(elements, "+1");
        let plusOneClickTarget2: { x: number; y: number } | null = null;
        if (plusOneSpan2.length > 0) {
            const btn = findSmallestContaining(
                elements,
                plusOneSpan2[0].rect,
                "tur_pointer_interact",
            );
            if (btn) {
                plusOneClickTarget2 = center(btn.rect);
            }
        }
        await assertClick(
            page,
            elements,
            "+1 button (2nd)",
            plusOneClickTarget2,
            canvasOffset,
        );
        await screenshot(page, "counter-click-plus1-again");

        // VERIFY: count is now 2
        elements = await getLayout(page);
        const countTwo = findTextSpans(elements, "Count: 2");
        if (countTwo.length > 0) {
            console.log('  PASS: "Count: 2" after clicking +1 twice');
            passed++;
        } else {
            console.log('  FAIL: "Count: 2" not found after clicking +1 twice');
            failed++;
        }

        // --- Step F: Click "-1" button ---
        console.log("\n--- Step F: Click -1 ---");
        elements = await getLayout(page);
        const minusOneSpan = findTextSpans(elements, "-1");
        let minusOneClickTarget: { x: number; y: number } | null = null;
        if (minusOneSpan.length > 0) {
            const btn = findSmallestContaining(
                elements,
                minusOneSpan[0].rect,
                "tur_pointer_interact",
            );
            if (btn) {
                minusOneClickTarget = center(btn.rect);
            }
        }
        if (minusOneClickTarget) {
            const px = Math.round(minusOneClickTarget.x + canvasOffset.x);
            const py = Math.round(minusOneClickTarget.y + canvasOffset.y);
            console.log(`  Clicking "-1 button" at (${px}, ${py})`);
            await page.mouse.click(px, py);
            await waitForRender(page);
            await screenshot(page, "counter-click-minus1");

            elements = await getLayout(page);
            const countBackToOne = findTextSpans(elements, "Count: 1");
            if (countBackToOne.length > 0) {
                console.log('  PASS: "Count: 1" after clicking -1 (was 2)');
                passed++;
            } else {
                console.log('  FAIL: "Count: 1" not found after clicking -1');
                failed++;
            }
        } else {
            console.log(
                '  SKIP: "-1" button not found in layout — cannot test decrement',
            );
            failed++;
            await screenshot(page, "counter-no-minus1");
        }

        console.log("\n=== Counter App Test Complete ===");

        printConsole(logs);

        const errorCount = logs.filter(
            (e) => e.type === "error" || e.type === "warning",
        ).length;
        if (errorCount > 0) {
            console.log(`\n${errorCount} warnings/errors found in console`);
        }

        console.log(`\n=== Results: ${passed} passed, ${failed} failed ===\n`);

        await browser.close();
    } finally {
        server.close();
    }

    if (failed > 0) {
        process.exit(1);
    }
}

main().catch((err) => {
    console.error(err);
    process.exit(1);
});
