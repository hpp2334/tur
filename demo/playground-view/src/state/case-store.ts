import {
    createModuleSource,
    createTextEditingController,
    createUndoController,
    createVirtualAppController,
    type KeyEvent,
    type Mutation,
    mutate,
    sleep,
    type Task,
} from "tur:std";
import { CASE_SOURCES, compileCase } from "../cases";
import { buildHighlightSpans } from "../cases/compile";
import {
    app$,
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
import type { CaseFileMap } from "./types";

// ---------------------------------------------------------------------------
// Case caches & last-compiled-source tracking
// ---------------------------------------------------------------------------

/** Per-case compiled module source cache (the child-instance sources the
 *  case store hands to `createModuleSource` on spawn). Primed at module
 *  eval so the first spawn needs no compile — but nothing runs until a
 *  controller binds (lazy). */
const caseSources = new Map<string, string>();

/** Per-case file cache: case name → { filename → current editor text }.
 *  Populated from CASE_SOURCES on first load; updated on each recompile. */
const caseFileCache = new Map<string, CaseFileMap>();

// Prime the caches synchronously so the first paint has something to render.
for (const name of CASE_NAMES) {
    const result = compileCase(CASE_SOURCES[name]);
    if (result.source != null) {
        caseSources.set(name, result.source);
    }
    caseFileCache.set(name, { ...CASE_SOURCES[name] });
}

// The last successfully-compiled file source per case — drives the `edited$`
// indicator (true when current editor text differs from this).
const lastCompiledFiles = new Map<string, CaseFileMap>();
for (const name of CASE_NAMES) {
    lastCompiledFiles.set(name, { ...CASE_SOURCES[name] });
}

let autoRunTask: Task<void> | null = null;

// ---------------------------------------------------------------------------
// Editor controller — closures reference forward-declared lifecycle fns
// (function declarations are hoisted within the same module).
// ---------------------------------------------------------------------------

export const editorCtrl = createTextEditingController({
    onInput: mutate((ctx, _text: string, _enter: boolean) => {
        editorCtrl.setSpansPreserveCursor(buildHighlightSpans(editorCtrl.text));
        saveCurrentFileText(ctx.get(selectedCase$), ctx.get(selectedFile$));
        if (ctx.get(autoRun$)) {
            // Debounce: cancel the previous delay, then wait 300ms — the
            // no-op rejection handler is the cancelled branch.
            autoRunTask?.cancel();
            autoRunTask = sleep(300);
            autoRunTask.promise.then(
                () => ctx.set(recompile),
                () => {},
            );
        }
    }),
    onKeyDown: mutate((ctx, ev: KeyEvent) => {
        if (ev.key === "s" && (ev.meta || ev.ctrl)) {
            ctx.set(recompile);
        }
    }),
});

/** Undo/redo history stack for the code editor. Passed to `Input` via the
 *  `undoController` prop so Cmd+Z / Cmd+Shift+Z work out of the box. */
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

/** Spawn (or re-spawn) the hosted child instance for a case: resolve its
 *  compiled module source (cache, then compile-on-demand), then swap `app$`
 *  to a fresh controller — the viewer's `VirtualAppView` unbinds the old
 *  controller and binds the new one in the same flush.
 *
 *  Controllers are created with `keepAlive: true` so the child SURVIVES
 *  viewer element churn (layout-mode switches, mobile tab swaps) — the
 *  element rebind is a no-op for a live child. The trade: swapping to a new
 *  controller must explicitly retire the old one (`destroy$` — always
 *  retires, regardless of `keepAlive`), or its child would leak. */
export const runCase: Mutation<[string], void> = mutate((ctx, name: string) => {
    if (!CASE_SOURCES[name]) return;
    let src = caseSources.get(name);
    if (src == null) {
        const files = caseFileCache.get(name) ?? { ...CASE_SOURCES[name] };
        const result = compileCase(files);
        if (result.error != null || result.source == null) {
            ctx.set(status$, "error");
            ctx.set(errorMsg$, result.error ?? "unknown error");
            return;
        }
        src = result.source;
        caseSources.set(name, src);
    }
    const previous = ctx.get(app$);
    if (previous != null) {
        ctx.set(previous.destroy$);
    }
    ctx.set(
        app$,
        createVirtualAppController({
            source: createModuleSource(src),
            keepAlive: true,
        }),
    );
});

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
        ctx.set(runCase, name);
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
    if (result.error != null || result.source == null) {
        ctx.set(status$, "error");
        ctx.set(errorMsg$, result.error ?? "unknown error");
        return;
    }
    caseSources.set(name, result.source);
    lastCompiledFiles.set(name, { ...files });
    ctx.set(lastCompiledAtMs$, Date.now());
    ctx.set(status$, "ready");
    ctx.set(errorMsg$, "");
    ctx.set(edited$, false);
    ctx.set(compileVersion$, ctx.get(compileVersion$) + 1);
    ctx.set(runCase, name);
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

/** Get the file names for a case (e.g. ["index.ts", "utils.ts"]). */
export function getCaseFileNames(name: string): string[] {
    return Object.keys(CASE_SOURCES[name] ?? {}).sort();
}

// Initialise editor with the first case. Must run after `editorCtrl` is bound.
const entryFiles = CASE_SOURCES[INITIAL_CASE] ?? {};
editorCtrl.setSpans(buildHighlightSpans(entryFiles["index.ts"] ?? ""));
