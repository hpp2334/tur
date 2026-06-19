import {
    createTextEditingController,
    type EdgyElement,
    get,
    type KeyEvent,
    mutate,
    set,
} from "@tur/edgy";
import { CASE_SOURCES, compileCase } from "../cases";
import { buildHighlightSpans } from "../cases/compile";
import {
    autoRun$,
    CASE_NAMES,
    edited$,
    errorMsg$,
    INITIAL_CASE,
    lastCompiledAtMs$,
    selectedCase$,
    status$,
} from "./sources";
import type { EditorController } from "./types";

// ---------------------------------------------------------------------------
// Case cache & last-compiled-source tracking
// ---------------------------------------------------------------------------

const caseComponents = new Map<string, EdgyElement>();

function compileIntoCache(name: string): void {
    const result = compileCase(CASE_SOURCES[name] ?? "");
    if (result.component) {
        caseComponents.set(name, result.component as EdgyElement);
    }
}

// Prime the cache synchronously so the first paint has something to render.
for (const name of CASE_NAMES) {
    compileIntoCache(name);
}

// The last successfully-compiled source per case — drives the `edited$`
// indicator (true when current editor text differs from this).
const lastCompiledText = new Map<string, string>();
for (const name of CASE_NAMES) {
    lastCompiledText.set(name, CASE_SOURCES[name] ?? "");
}

let autoRunTimer: ReturnType<typeof setTimeout> | null = null;

// ---------------------------------------------------------------------------
// Editor controller — closures reference forward-declared lifecycle fns
// (function declarations are hoisted within the same module).
// ---------------------------------------------------------------------------

export const editorCtrl = createTextEditingController({
    onInput: mutate((_ctx, _text: string, _enter: boolean) => {
        editorCtrl.setSpansPreserveCursor(buildHighlightSpans(editorCtrl.text));
        refreshEditedState();
        if (get(autoRun$)) {
            if (autoRunTimer) clearTimeout(autoRunTimer);
            autoRunTimer = setTimeout(() => recompile(), 300);
        }
    }),
    onKeyDown: mutate((_ctx, ev: KeyEvent) => {
        if (ev.key === "s" && (ev.meta || ev.ctrl)) {
            recompile();
        }
    }),
}) as unknown as EditorController;

function refreshEditedState(): void {
    const name = get(selectedCase$);
    const baseline = lastCompiledText.get(name) ?? "";
    set(edited$, editorCtrl.text !== baseline);
}

// ---------------------------------------------------------------------------
// Case lifecycle
// ---------------------------------------------------------------------------

export function loadCase(name: string): void {
    if (!CASE_SOURCES[name]) return;
    set(selectedCase$, name);
    editorCtrl.setSpans(buildHighlightSpans(CASE_SOURCES[name]));
    set(status$, "ready");
    set(errorMsg$, "");
    refreshEditedState();
}

export function recompile(): void {
    if (autoRunTimer) {
        clearTimeout(autoRunTimer);
        autoRunTimer = null;
    }
    const name = get(selectedCase$);
    const result = compileCase(editorCtrl.text);
    if (result.error || !result.component) {
        set(status$, "error");
        set(errorMsg$, result.error ?? "unknown error");
        return;
    }
    caseComponents.set(name, result.component as EdgyElement);
    lastCompiledText.set(name, editorCtrl.text);
    set(lastCompiledAtMs$, Date.now());
    set(status$, "ready");
    set(errorMsg$, "");
    set(edited$, false);
}

export function resetCase(): void {
    const name = get(selectedCase$);
    const original = CASE_SOURCES[name] ?? "";
    editorCtrl.setSpans(buildHighlightSpans(original));
    recompile();
}

/** Look up the cached component handle for a case (or undefined). Used by
 *  the viewer pane to render the active case. */
export function getCaseComponent(name: string): EdgyElement | undefined {
    return caseComponents.get(name);
}

// Initialise editor with the first case. Must run after `editorCtrl` is bound.
editorCtrl.setSpans(buildHighlightSpans(CASE_SOURCES[INITIAL_CASE] ?? ""));
