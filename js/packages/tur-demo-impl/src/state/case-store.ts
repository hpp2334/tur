import {
    createTextEditingController,
    createUndoController,
    type Element,
    get,
    type KeyEvent,
    launch,
    mutate,
    set,
    sleep,
    type Task,
} from "builtin:tur/std";
import { CASE_SOURCES, compileCase } from "../cases";
import { buildHighlightSpans } from "../cases/compile";
import {
    autoRun$,
    CASE_NAMES,
    compileVersion$,
    edited$,
    errorMsg$,
    INITIAL_CASE,
    isMobile$,
    lastCompiledAtMs$,
    mobileTab$,
    selectedCase$,
    selectedFile$,
    status$,
} from "./sources";
import { triggerFadeIn } from "./transitions";
import type { CaseFileMap, EditorController } from "./types";

// ---------------------------------------------------------------------------
// Case cache & last-compiled-source tracking
// ---------------------------------------------------------------------------

const caseViews = new Map<string, Element>();

/** Per-case file cache: case name → { filename → current editor text }.
 *  Populated from CASE_SOURCES on first load; updated on each recompile. */
const caseFileCache = new Map<string, CaseFileMap>();

function compileIntoCache(name: string): void {
    const files = CASE_SOURCES[name];
    if (!files) return;
    const result = compileCase(files);
    if (result.view) {
        caseViews.set(name, result.view as Element);
    }
}

// Prime the cache synchronously so the first paint has something to render.
for (const name of CASE_NAMES) {
    compileIntoCache(name);
    caseFileCache.set(name, { ...CASE_SOURCES[name] });
}

// The last successfully-compiled file source per case — drives the `edited$`
// indicator (true when current editor text differs from this).
const lastCompiledFiles = new Map<string, CaseFileMap>();
for (const name of CASE_NAMES) {
    lastCompiledFiles.set(name, { ...CASE_SOURCES[name] });
}

let autoRunTask: Task | null = null;

// ---------------------------------------------------------------------------
// Editor controller — closures reference forward-declared lifecycle fns
// (function declarations are hoisted within the same module).
// ---------------------------------------------------------------------------

export const editorCtrl = createTextEditingController({
    onInput: mutate((_ctx, _text: string, _enter: boolean) => {
        editorCtrl.setSpansPreserveCursor(buildHighlightSpans(editorCtrl.text));
        saveCurrentFileText();
        refreshEditedState();
        if (get(autoRun$)) {
            autoRunTask?.cancel();
            autoRunTask = launch(function* () {
                yield sleep(300);
                recompile();
            });
        }
    }),
    onKeyDown: mutate((_ctx, ev: KeyEvent) => {
        if (ev.key === "s" && (ev.meta || ev.ctrl)) {
            recompile();
        }
    }),
});

/** Undo/redo history stack for the code editor. Passed to `Input` via
 *  the `undoController` prop so Cmd+Z / Cmd+Shift+Z work out of the box. */
export const editorUndo = createUndoController();

/** Save the current editor text back to the per-case file cache. */
function saveCurrentFileText(): void {
    const name = get(selectedCase$);
    const filename = get(selectedFile$);
    let cache = caseFileCache.get(name);
    if (!cache) {
        cache = {};
        caseFileCache.set(name, cache);
    }
    cache[filename] = editorCtrl.text;
}

function refreshEditedState(): void {
    const name = get(selectedCase$);
    const filename = get(selectedFile$);
    const baseline = lastCompiledFiles.get(name)?.[filename] ?? "";
    set(edited$, editorCtrl.text !== baseline);
}

// ---------------------------------------------------------------------------
// Case lifecycle
// ---------------------------------------------------------------------------

export function loadCase(name: string): void {
    if (!CASE_SOURCES[name]) return;
    set(selectedCase$, name);
    set(selectedFile$, "index.ts");

    // On mobile, jump to the viewer so the user sees the rendered case right
    // after picking it from the Cases tab.
    if (get(isMobile$)) {
        set(mobileTab$, "view");
    }

    // Ensure file cache is populated.
    if (!caseFileCache.has(name)) {
        caseFileCache.set(name, { ...CASE_SOURCES[name] });
    }

    const files = caseFileCache.get(name) ?? {};
    const entryText = files["index.ts"] ?? "";
    editorCtrl.setSpans(buildHighlightSpans(entryText));
    set(status$, "ready");
    set(errorMsg$, "");
    refreshEditedState();
    triggerFadeIn();
}

/** Switch to a different file within the current case. Saves the current
 *  editor text, loads the new file. */
export function selectFile(filename: string): void {
    saveCurrentFileText();
    const name = get(selectedCase$);
    const files = caseFileCache.get(name) ?? {};
    const text = files[filename] ?? "";
    set(selectedFile$, filename);
    editorCtrl.setSpans(buildHighlightSpans(text));
    refreshEditedState();
}

export function recompile(): void {
    autoRunTask?.cancel();
    autoRunTask = null;
    const name = get(selectedCase$);

    // Save current editor text to the file cache before compiling.
    saveCurrentFileText();
    const files = caseFileCache.get(name) ?? {};

    const result = compileCase(files);
    if (result.error || !result.view) {
        set(status$, "error");
        set(errorMsg$, result.error ?? "unknown error");
        return;
    }
    caseViews.set(name, result.view as Element);
    lastCompiledFiles.set(name, { ...files });
    set(lastCompiledAtMs$, Date.now());
    set(status$, "ready");
    set(errorMsg$, "");
    set(edited$, false);
    set(compileVersion$, get(compileVersion$) + 1);
    triggerFadeIn();
}

export function resetCase(): void {
    const name = get(selectedCase$);
    const original = CASE_SOURCES[name] ?? {};
    caseFileCache.set(name, { ...original });
    lastCompiledFiles.set(name, { ...original });
    const filename = get(selectedFile$);
    editorCtrl.setSpans(buildHighlightSpans(original[filename] ?? ""));
    recompile();
}

/** Look up the cached view handle for a case (or undefined). Used by
 *  the viewer pane to render the active case. */
export function getCaseView(name: string): Element | undefined {
    return caseViews.get(name);
}

/** Get the file names for a case (e.g. ["index.ts", "utils.ts"]). */
export function getCaseFileNames(name: string): string[] {
    return Object.keys(CASE_SOURCES[name] ?? {}).sort();
}

// Initialise editor with the first case. Must run after `editorCtrl` is bound.
const entryFiles = CASE_SOURCES[INITIAL_CASE] ?? {};
editorCtrl.setSpans(buildHighlightSpans(entryFiles["index.ts"] ?? ""));
