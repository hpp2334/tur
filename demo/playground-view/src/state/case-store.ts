import {
    createTextEditingController,
    createUndoController,
    type Element,
    type KeyEvent,
    launch,
    type Mutation,
    mutate,
    sleep,
    type Task,
} from "tur:std";
import { CASE_SOURCES, compileCase, takePublishedView } from "../cases";
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

/** Per-case cleanup returned by the last invoked `start()` (the module
 *  lifecycle contract, in-realm form). Runs before that case's next
 *  recompile so controllers / animation loops don't leak across runs. */
const caseCleanups = new Map<string, () => void>();

/** Per-case file cache: case name → { filename → current editor text }.
 *  Populated from CASE_SOURCES on first load; updated on each recompile. */
const caseFileCache = new Map<string, CaseFileMap>();

/** Run a compiled case's `start()`: tear down the previous run's cleanup,
 *  invoke `start`, then drain the view it published via `setCaseView`. */
function invokeCaseStart(name: string, start: () => (() => void) | void): void {
    caseCleanups.get(name)?.();
    caseCleanups.delete(name);
    const cleanup = start();
    if (typeof cleanup === "function") {
        caseCleanups.set(name, cleanup);
    }
    const view = takePublishedView();
    if (view != null) {
        caseViews.set(name, view as Element);
    }
}

function compileIntoCache(name: string): void {
    const files = CASE_SOURCES[name];
    if (!files) return;
    const result = compileCase(files);
    if (result.start) {
        invokeCaseStart(name, result.start);
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
    onInput: mutate((ctx, _text: string, _enter: boolean) => {
        editorCtrl.setSpansPreserveCursor(buildHighlightSpans(editorCtrl.text));
        saveCurrentFileText(ctx.get(selectedCase$), ctx.get(selectedFile$));
        if (ctx.get(autoRun$)) {
            autoRunTask?.cancel();
            autoRunTask = launch(function* () {
                yield sleep(300);
                ctx.set(recompile);
            });
        }
    }),
    onKeyDown: mutate((ctx, ev: KeyEvent) => {
        if (ev.key === "s" && (ev.meta || ev.ctrl)) {
            ctx.set(recompile);
        }
    }),
});

/** Undo/redo history stack for the code editor. Passed to `Input` via
 *  the `undoController` prop so Cmd+Z / Cmd+Shift+Z work out of the box. */
export const editorUndo = createUndoController();

/** Save the current editor text back to the per-case file cache. Pure cache
 *  write — callers read the case/file atoms from their ctx and pass values. */
function saveCurrentFileText(name: string, filename: string): void {
    let cache = caseFileCache.get(name);
    if (!cache) {
        cache = {};
        caseFileCache.set(name, cache);
    }
    cache[filename] = editorCtrl.text;
}

/** Pure: does the current editor text differ from the last-compiled source
 *  for this case+file? Callers `ctx.set(edited$, …)` with the result. */
function isEdited(name: string, filename: string): boolean {
    const baseline = lastCompiledFiles.get(name)?.[filename] ?? "";
    return editorCtrl.text !== baseline;
}

// ---------------------------------------------------------------------------
// Case lifecycle — every action is a `mutate` declaration; callers dispatch
// via `ctx.set(action, ...args)` (same-flush: the engine's fixed-point loop
// drains newly-queued mutations in the next iteration).
// ---------------------------------------------------------------------------

export const loadCase: Mutation<[string], void> = mutate(
    (ctx, name: string) => {
        if (!CASE_SOURCES[name]) return;
        ctx.set(selectedCase$, name);
        ctx.set(selectedFile$, "index.ts");

        // On mobile, jump to the viewer so the user sees the rendered case
        // right after picking it from the Cases tab.
        if (ctx.get(isMobile$)) {
            ctx.set(mobileTab$, "view");
        }

        // Ensure file cache is populated.
        if (!caseFileCache.has(name)) {
            caseFileCache.set(name, { ...CASE_SOURCES[name] });
        }

        const files = caseFileCache.get(name) ?? {};
        const entryText = files["index.ts"] ?? "";
        editorCtrl.setSpans(buildHighlightSpans(entryText));
        ctx.set(status$, "ready");
        ctx.set(errorMsg$, "");
        ctx.set(edited$, isEdited(name, "index.ts"));
        triggerFadeIn();
    },
);

/** Switch to a different file within the current case. Saves the current
 *  editor text, loads the new file. */
export const selectFile: Mutation<[string], void> = mutate(
    (ctx, filename: string) => {
        saveCurrentFileText(ctx.get(selectedCase$), ctx.get(selectedFile$));
        const files = caseFileCache.get(ctx.get(selectedCase$)) ?? {};
        const text = files[filename] ?? "";
        ctx.set(selectedFile$, filename);
        editorCtrl.setSpans(buildHighlightSpans(text));
        ctx.set(edited$, isEdited(ctx.get(selectedCase$), filename));
    },
);

export const recompile: Mutation<[], void> = mutate((ctx) => {
    autoRunTask?.cancel();
    autoRunTask = null;
    const name = ctx.get(selectedCase$);

    // Save current editor text to the file cache before compiling.
    saveCurrentFileText(name, ctx.get(selectedFile$));
    const files = caseFileCache.get(name) ?? {};

    const result = compileCase(files);
    if (result.error || !result.start) {
        ctx.set(status$, "error");
        ctx.set(errorMsg$, result.error ?? "unknown error");
        return;
    }
    invokeCaseStart(name, result.start);
    lastCompiledFiles.set(name, { ...files });
    ctx.set(lastCompiledAtMs$, Date.now());
    ctx.set(status$, "ready");
    ctx.set(errorMsg$, "");
    ctx.set(edited$, false);
    ctx.set(compileVersion$, ctx.get(compileVersion$) + 1);
    triggerFadeIn();
});

export const resetCase: Mutation<[], void> = mutate((ctx) => {
    const name = ctx.get(selectedCase$);
    const original = CASE_SOURCES[name] ?? {};
    caseFileCache.set(name, { ...original });
    lastCompiledFiles.set(name, { ...original });
    editorCtrl.setSpans(
        buildHighlightSpans(original[ctx.get(selectedFile$)] ?? ""),
    );
    ctx.set(recompile);
});

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
