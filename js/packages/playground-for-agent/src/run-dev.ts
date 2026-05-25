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
            {
                name: "subjectAltName",
                altNames: [{ type: 2, value: "localhost" }],
            },
        ],
    };
    const { cert, private: key } = await generateCert(attrs, options);

    return new Promise((resolve, reject) => {
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
        (e) => e.label === text && e.type === "tur_text_span",
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
        if (btn && btn.rect.y > 380) {
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
        if (btn && btn.rect.y < 200) {
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
        (e) => e.type === "tur_input" && e.rect.w > 300 && e.rect.h < 50,
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
        if (row && row.rect.w > 400) return center(row.rect);
    }
    return null;
}

function hasModalOpen(elements: ElementInfo[]): boolean {
    return elements.some(
        (e) => e.label === "Add New Task" && e.type === "tur_text_span",
    );
}

function getTaskTexts(elements: ElementInfo[]): string[] {
    const taskItems: string[] = [];
    for (const e of elements) {
        if (
            e.type === "tur_text_span" &&
            e.label &&
            e.label !== "v" &&
            e.label !== "x" &&
            e.rect.x > 250 &&
            e.rect.y > 180 &&
            e.rect.y < 550
        ) {
            const hasDeleteBtn = elements.some(
                (d) =>
                    d.type === "tur_pointer_interact" &&
                    d.rect.w <= 40 &&
                    d.rect.h <= 40 &&
                    d.rect.x > 700 &&
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
) {
    if (!target) {
        throw new Error(`ASSERT FAIL: could not find "${label}" to click`);
    }
    console.log(
        `  Clicking "${label}" at (${Math.round(target.x)}, ${Math.round(target.y)})`,
    );
    await page.mouse.click(target.x, target.y);
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

        await page.waitForFunction(
            () => (window as Record<string, unknown>).turDemo !== undefined,
            { timeout: 10000 },
        );
        await waitForRender(page, 3000);

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

        // --- Step 2: Click "+ New Task" button ---
        console.log("\n--- Step 2: Click + New Task ---");
        elements = await getLayout(page);
        const newTaskBtn = findHeaderNewTaskButton(elements);
        await assertClick(page, elements, "+ New Task (header)", newTaskBtn);
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

        // --- Step 3: Type in the modal's title input ---
        console.log("\n--- Step 3: Type in input ---");
        elements = await getLayout(page);
        const titleInput = findModalInput(elements, 0);
        if (titleInput) {
            const c = center(titleInput.rect);
            console.log(
                `  Clicking title input at (${Math.round(c.x)}, ${Math.round(c.y)})`,
            );
            await page.mouse.click(c.x, c.y);
            await waitForRender(page, 200);
            await page.keyboard.type("Buy groceries");
        }
        await waitForRender(page);
        await screenshot(page, "type-buy-groceries");

        // VERIFY: input has text
        elements = await getLayout(page);
        const typedInput = findElement(
            elements,
            (e) => e.type === "tur_input" && e.label.includes("Buy groceries"),
        );
        if (typedInput) {
            console.log(`  PASS: input shows "${typedInput.label}"`);
            passed++;
        } else {
            const allInputs = findAll(elements, (e) => e.type === "tur_input");
            console.log(
                `  FAIL: input does not show "Buy groceries". Inputs: ${allInputs.map((e) => `"${e.label}"`).join(", ")}`,
            );
            failed++;
        }

        // --- Step 4: Click "Add Task" button (in modal) ---
        console.log("\n--- Step 4: Click Add Task (modal) ---");
        elements = await getLayout(page);
        const addTaskBtn = findModalAddTaskButton(elements);
        await assertClick(page, elements, "Add Task (modal)", addTaskBtn);
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

        // --- Step 5: Add another task ---
        console.log("\n--- Step 5: Add second task 'Review PR' ---");
        elements = await getLayout(page);
        const newTaskBtn2 = findHeaderNewTaskButton(elements);
        await assertClick(page, elements, "+ New Task (header)", newTaskBtn2);
        await screenshot(page, "open-modal-again");

        elements = await getLayout(page);
        if (!hasModalOpen(elements)) {
            console.log("  FAIL: modal did not open for second task");
            failed++;
        } else {
            const input2 = findModalInput(elements, 0);
            if (input2) {
                const c = center(input2.rect);
                console.log(
                    `  Clicking title input at (${Math.round(c.x)}, ${Math.round(c.y)})`,
                );
                await page.mouse.click(c.x, c.y);
                await waitForRender(page, 200);
                await page.keyboard.type("Review PR");
            }
            await waitForRender(page);

            elements = await getLayout(page);
            const addTaskBtn2 = findModalAddTaskButton(elements);
            await assertClick(page, elements, "Add Task (modal)", addTaskBtn2);
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

        // --- Step 6: Toggle checkbox on "Build tur engine" ---
        console.log("\n--- Step 6: Toggle checkbox ---");
        elements = await getLayout(page);
        const checkbox = findCheckboxForItem(elements, "Build tur engine");
        await assertClick(
            page,
            elements,
            "checkbox for Build tur engine",
            checkbox,
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
                    e.type === "tur_text_span" &&
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

        // --- Step 7: Delete "Write documentation" ---
        console.log("\n--- Step 7: Delete 'Write documentation' ---");
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

        // --- Step 8: Click on "Learn Rust" to select it ---
        console.log("\n--- Step 8: Select 'Learn Rust' ---");
        elements = await getLayout(page);
        const learnRustRow = findItemRow(elements, "Learn Rust");
        await assertClick(page, elements, "row for Learn Rust", learnRustRow);
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
