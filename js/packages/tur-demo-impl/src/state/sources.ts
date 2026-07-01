import { type Atom, set, source } from "@tur/edgy";
import { CASE_SOURCES } from "../cases";
import type { LayoutMode } from "./types";

// ---------------------------------------------------------------------------
// Whitelist & case ordering
// ---------------------------------------------------------------------------

const WHITELIST = new Set([
    "counter",
    "todolist",
    "complex-animation",
    "lazy-list-virtualized",
    "lazy-list-var-sizes",
    "jigsaw-puzzle",
    "countdown",
    "github-viewer",
    "lifecycle-hooks",
]);

export const CASE_NAMES = Object.keys(CASE_SOURCES)
    .filter((name) => WHITELIST.has(name))
    .sort();
export const INITIAL_CASE = CASE_NAMES.includes("counter")
    ? "counter"
    : (CASE_NAMES[0] ?? "");

// ---------------------------------------------------------------------------
// Reactive state
// ---------------------------------------------------------------------------

export const selectedCase$ = source<string>(INITIAL_CASE);
export const selectedFile$ = source<string>("index.ts");
export const status$ = source<"ready" | "error">("ready");
export const errorMsg$ = source("");
export const edited$ = source(false);

// Bumped on every successful recompile so consumers keyed on the active case
// (e.g. the viewer) re-read the cached view handle.
export const compileVersion$ = source<number>(0);

// Per-element hover state (single source per interactive group, not per
// instance — keeps the subscription graph flat).
export const hoveredCase$ = source<string | null>(null);
export const hoveredFile$ = source<string | null>(null);
export const runHovered$ = source(false);
export const resetHovered$ = source(false);
export const layoutHovered$ = source<string | null>(null);

// User preferences.
export const autoRun$ = source(true);
export const layoutMode$ = source<LayoutMode>("split");

// Draggable divider widths (pixel-based for 1:1 mouse tracking).
// `sidebarWidth$` is the sidebar's pixel width; `editorWidth$` is the editor
// pane's pixel width in split mode (the viewer pane is `Expanded` and fills
// the remaining space). Updated by the divider drag handlers in
// `views/divider.ts`.
export const sidebarWidth$ = source(200);
export const editorWidth$ = source(600);

// "Compiled Xs ago" — `now$` ticks every 5s so the relative timestamp in the
// status bar stays fresh without manual refresh.
export const lastCompiledAtMs$ = source<number>(Date.now());
export const now$: Atom<number> = source<number>(Date.now());
setInterval(() => set(now$, Date.now()), 5000);
