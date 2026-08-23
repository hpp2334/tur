import {
    derive,
    launch,
    type Mutation,
    mutate,
    type Source,
    sleep,
    source,
    type Task,
    type ViewportSize,
    viewportSize$,
} from "tur:std";
import { CASE_SOURCES } from "../cases";
import type { LayoutMode, MobileTab } from "./types";

// ---------------------------------------------------------------------------
// Whitelist & case ordering
// ---------------------------------------------------------------------------

const WHITELIST = new Set([
    "counter",
    "todolist",
    "complex-animation",
    "composited-transform-anchor-playground",
    "implicit-animations",
    "lazy-list-virtualized",
    "lazy-list-var-sizes",
    "lazy-grid-gallery",
    "lazy-grid-basic",
    "lazy-grid-scroll",
    "grid-gallery",
    "grid-basic",
    "grid-aspect",
    "jigsaw-puzzle",
    "countdown",
    "github-viewer",
    "text-demo",
    "password-input",
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

// Responsive layout. `viewportSize$` is engine-provided (backed by the
// canvas resize handler; readable through the mounted-store ctx). Below
// 720px CSS width the playground switches to the mobile single-pane +
// bottom-tab layout (see views/shell.ts).
export const isMobile$ = derive(
    (ctx) => ctx.get<ViewportSize>(viewportSize$).width < 720,
);

// Active pane on mobile (bottom tab bar). Desktop uses `layoutMode$` instead.
export const mobileTab$ = source<MobileTab>("edit");

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
export const now$: Source<number> = source<number>(Date.now());

// The ticker is a mutation so its `launch` loop can capture the mutation ctx
// (the store-bound writer) — there is no module store to write through. The
// entry point dispatches it once after `mount`; the returned module cleanup
// cancels the task so a reload doesn't leak the previous loop.
let nowTask: Task | null = null;

export const startNowTicker: Mutation<[], void> = mutate((ctx) => {
    nowTask?.cancel();
    nowTask = launch(function* () {
        for (;;) {
            yield sleep(5000);
            ctx.set(now$, Date.now());
        }
    });
});

/** Cancel the `now$` ticker (module cleanup, run by the entry point). */
export function stopNowTicker(): void {
    nowTask?.cancel();
    nowTask = null;
}
