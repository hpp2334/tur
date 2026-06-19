import {
    Column,
    Condition,
    Container,
    CrossAxisAlignment,
    component,
    createTextEditingController,
    derive,
    type EdgyComponent,
    type EdgyElement,
    Expanded,
    get,
    InputEdgy,
    type KeyEvent,
    MainAxisAlignment,
    MainAxisSize,
    mutate,
    PointerInteract,
    Positioned,
    Row,
    render,
    ScrollView,
    SizedBox,
    Stack,
    Switch,
    set,
    source,
    Text,
} from "@tur/edgy";
import { CASE_SOURCES } from "./cases-generated";
import { buildHighlightSpans, compileCase } from "./compile";
import { tokens } from "./tokens";

interface EditorController {
    setSpans(spans: Array<{ content: string; color?: unknown }>): void;
    setSpansPreserveCursor(
        spans: Array<{ content: string; color?: unknown }>,
    ): void;
    readonly text: string;
}

// Expose the bundled @tur/edgy on `globalThis.TurEdgy` so that case sources
// (eval'd at runtime after import-rewriting) share this same bridge instance.
import * as Edgy from "@tur/edgy";

(globalThis as unknown as { TurEdgy: unknown }).TurEdgy = Edgy;

// ---------------------------------------------------------------------------
// Constants & reactive state
// ---------------------------------------------------------------------------

const WHITELIST = new Set([
    "counter",
    "clickable-text",
    "container-basic",
    "column-basic",
    "todolist",
]);

const CASE_NAMES = Object.keys(CASE_SOURCES)
    .filter((name) => WHITELIST.has(name))
    .sort();
const INITIAL_CASE = CASE_NAMES.includes("counter")
    ? "counter"
    : (CASE_NAMES[0] ?? "");

type LayoutMode = "split" | "editor" | "viewer";

const selectedCase$ = source<string>(INITIAL_CASE);
const status$ = source<"ready" | "error">("ready");
const errorMsg$ = source("");
const edited$ = source(false);

// Per-element hover state (single source per interactive group, not per
// instance — keeps the subscription graph flat).
const hoveredCase$ = source<string | null>(null);
const runHovered$ = source(false);
const resetHovered$ = source(false);
const layoutHovered$ = source<string | null>(null);

// User preferences.
const autoRun$ = source(true);
const layoutMode$ = source<LayoutMode>("split");

// "Compiled Xs ago" — `now$` ticks every 5s so the relative timestamp in the
// status bar stays fresh without manual refresh.
const lastCompiledAtMs$ = source<number>(Date.now());
const now$ = source<number>(Date.now());
setInterval(() => set(now$, Date.now()), 5000);

// ---------------------------------------------------------------------------
// Case cache & editor controller
// ---------------------------------------------------------------------------

const caseComponents = new Map<string, () => EdgyElement>();
function compileIntoCache(name: string): void {
    const result = compileCase(CASE_SOURCES[name] ?? "");
    if (result.component) {
        caseComponents.set(name, result.component as () => EdgyElement);
    }
}
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

const editorCtrl = createTextEditingController({
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

function loadCase(name: string): void {
    if (!CASE_SOURCES[name]) return;
    set(selectedCase$, name);
    editorCtrl.setSpans(buildHighlightSpans(CASE_SOURCES[name]));
    set(status$, "ready");
    set(errorMsg$, "");
    refreshEditedState();
}

function recompile(): void {
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
    caseComponents.set(name, result.component as () => EdgyElement);
    lastCompiledText.set(name, editorCtrl.text);
    set(lastCompiledAtMs$, Date.now());
    set(status$, "ready");
    set(errorMsg$, "");
    set(edited$, false);
    // Rebuild the shell so the viewer's component Switch picks up the new
    // factory for this case.
    render(Shell);
}

function resetCase(): void {
    const name = get(selectedCase$);
    const original = CASE_SOURCES[name] ?? "";
    editorCtrl.setSpans(buildHighlightSpans(original));
    recompile();
}

// Initialise editor with the first case.
editorCtrl.setSpans(buildHighlightSpans(CASE_SOURCES[INITIAL_CASE] ?? ""));

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function relativeTime(ms: number, nowMs: number): string {
    const diff = Math.max(0, nowMs - ms);
    const s = Math.floor(diff / 1000);
    if (s < 5) return "just now";
    if (s < 60) return `${s}s ago`;
    const m = Math.floor(s / 60);
    if (m < 60) return `${m}m ago`;
    const h = Math.floor(m / 60);
    return `${h}h ago`;
}

function layoutFlex(who: "editor" | "viewer", mode: LayoutMode): number {
    if (mode === "editor") return who === "editor" ? 2 : 1;
    if (mode === "viewer") return who === "editor" ? 1 : 2;
    return 1;
}

// ---------------------------------------------------------------------------
// UI primitives
// ---------------------------------------------------------------------------

function Placeholder(): EdgyElement {
    return Container({
        color: tokens.bg.viewer,
        alignment: 4, // Alignment.Center
        children: [
            Text({
                text: "(no case)",
                fontSize: 13,
                color: tokens.text.tertiary,
            }),
        ],
    });
}

function NavItem(name: string): EdgyElement {
    return PointerInteract({
        onClick: mutate(() => loadCase(name)),
        onPointerEnter: mutate(() => set(hoveredCase$, name)),
        onPointerExit: mutate(() => set(hoveredCase$, null)),
        child: Container({
            padding: 8,
            color: derive(() => {
                const selected = get(selectedCase$) === name;
                const hovered = get(hoveredCase$) === name;
                if (selected)
                    return hovered
                        ? tokens.bg.selectedHover
                        : tokens.bg.selected;
                return hovered ? tokens.bg.hover : tokens.bg.panel;
            }),
            children: [
                Row({
                    children: [
                        Text({
                            text: name,
                            fontSize: 13,
                            color: derive(() =>
                                get(selectedCase$) === name
                                    ? tokens.text.primary
                                    : get(hoveredCase$) === name
                                      ? tokens.text.primary
                                      : tokens.text.body,
                            ),
                        }),
                        // Edited indicator — small coral dot when this case's
                        // editor text differs from its last-compiled version.
                        Condition({
                            condition: derive(
                                () =>
                                    get(edited$) && get(selectedCase$) === name,
                            ),
                            child: Container({
                                width: 6,
                                height: 6,
                                borderRadius: 999,
                                color: tokens.accent.complement,
                            }),
                            elseChild: SizedBox({ width: 0, height: 0 }),
                        }),
                    ],
                }),
            ],
        }),
    });
}

function Sidebar(): EdgyElement {
    return Container({
        width: 200,
        color: tokens.bg.panel,
        children: [
            Column({
                crossAlignment: CrossAxisAlignment.Stretch,
                children: [
                    SizedBox({ height: 8 }),
                    Expanded({
                        child: ScrollView({
                            child: Column({
                                crossAlignment: CrossAxisAlignment.Start,
                                children: CASE_NAMES.map((name) =>
                                    NavItem(name),
                                ),
                            }),
                        }),
                    }),
                ],
            }),
        ],
    });
}

// --- Toolbar widgets --------------------------------------------------------

function RunButton(): EdgyElement {
    return PointerInteract({
        onClick: mutate(recompile),
        onPointerEnter: mutate(() => set(runHovered$, true)),
        onPointerExit: mutate(() => set(runHovered$, false)),
        child: Container({
            padding: 6,
            borderRadius: 6,
            color: derive(() =>
                get(runHovered$)
                    ? tokens.bg.button.primaryHover
                    : tokens.bg.button.primary,
            ),
            children: [
                Row({
                    mainAxisSize: MainAxisSize.Min,
                    children: [
                        Text({
                            text: "\u25B6", // ▶
                            fontSize: 9,
                            color: tokens.text.onAccent,
                        }),
                        SizedBox({ width: 4 }),
                        Text({
                            text: "Run",
                            fontSize: 12,
                            color: tokens.text.onAccent,
                        }),
                    ],
                }),
            ],
        }),
    });
}

function ResetButton(): EdgyElement {
    return PointerInteract({
        onClick: mutate(resetCase),
        onPointerEnter: mutate(() => set(resetHovered$, true)),
        onPointerExit: mutate(() => set(resetHovered$, false)),
        child: Container({
            padding: 6,
            borderRadius: 6,
            color: derive(() =>
                get(resetHovered$) ? tokens.bg.hover : tokens.bg.button.ghost,
            ),
            children: [
                Row({
                    mainAxisSize: MainAxisSize.Min,
                    children: [
                        Text({
                            text: "\u21BA", // ↺
                            fontSize: 12,
                            color: tokens.text.secondary,
                        }),
                        SizedBox({ width: 4 }),
                        Text({
                            text: "Reset",
                            fontSize: 12,
                            color: tokens.text.secondary,
                        }),
                    ],
                }),
            ],
        }),
    });
}

function AutoRunToggle(): EdgyElement {
    return Row({
        mainAxisSize: MainAxisSize.Min,
        children: [
            Text({
                text: "auto",
                fontSize: 11,
                color: tokens.text.secondary,
            }),
            SizedBox({ width: 6 }),
            PointerInteract({
                onClick: mutate(() => set(autoRun$, !get(autoRun$))),
                child: Container({
                    width: 28,
                    height: 16,
                    borderRadius: 999,
                    color: derive(() =>
                        get(autoRun$)
                            ? tokens.bg.button.primary
                            : tokens.bg.hover,
                    ),
                    children: [
                        Stack({
                            children: [
                                Positioned({
                                    top: 2,
                                    left: derive(() =>
                                        get(autoRun$) ? 14 : 2,
                                    ),
                                    child: Container({
                                        width: 12,
                                        height: 12,
                                        borderRadius: 999,
                                        color: tokens.text.inverse,
                                    }),
                                }),
                            ],
                        }),
                    ],
                }),
            }),
        ],
    });
}

function LayoutButton(mode: LayoutMode, label: string): EdgyElement {
    return PointerInteract({
        onClick: mutate(() => set(layoutMode$, mode)),
        onPointerEnter: mutate(() => set(layoutHovered$, mode)),
        onPointerExit: mutate(() => set(layoutHovered$, null)),
        child: Container({
            padding: 6,
            color: derive(() => {
                const selected = get(layoutMode$) === mode;
                const hovered = get(layoutHovered$) === mode;
                if (selected) return tokens.bg.controlSelected;
                if (hovered) return tokens.bg.controlTrayHover;
                return tokens.bg.controlTray;
            }),
            children: [
                Text({
                    text: label,
                    fontSize: 11,
                    color: derive(() =>
                        get(layoutMode$) === mode
                            ? tokens.text.primary
                            : tokens.text.secondary,
                    ),
                }),
            ],
        }),
    });
}

function LayoutControl(): EdgyElement {
    return Container({
        color: tokens.bg.controlTray,
        children: [
            Row({
                mainAxisSize: MainAxisSize.Min,
                children: [
                    LayoutButton("split", "Split"),
                    LayoutButton("editor", "Edit"),
                    LayoutButton("viewer", "View"),
                ],
            }),
        ],
    });
}

function Toolbar(): EdgyElement {
    return Container({
        color: tokens.bg.elevated,
        borderColor: tokens.border.subtle,
        borderWidth: 1,
        children: [
            Row({
                children: [
                    // Brand.
                    Container({
                        padding: 12,
                        children: [
                            Row({
                                mainAxisSize: MainAxisSize.Min,
                                children: [
                                    Text({
                                        text: "tur",
                                        fontSize: 14,
                                        color: tokens.accent.solid,
                                    }),
                                    SizedBox({ width: 4 }),
                                    Text({
                                        text: "playground",
                                        fontSize: 11,
                                        color: tokens.text.secondary,
                                    }),
                                ],
                            }),
                        ],
                    }),
                    // Case name (center, expands).
                    Expanded({
                        child: Container({
                            padding: 12,
                            children: [
                                Text({
                                    text: derive(() => get(selectedCase$)),
                                    fontSize: 13,
                                    color: tokens.text.body,
                                }),
                            ],
                        }),
                    }),
                    // Actions.
                    Container({
                        padding: 12,
                        children: [
                            Row({
                                mainAxisSize: MainAxisSize.Min,
                                children: [
                                    RunButton(),
                                    SizedBox({ width: 6 }),
                                    ResetButton(),
                                    SizedBox({ width: 12 }),
                                    AutoRunToggle(),
                                    SizedBox({ width: 12 }),
                                    LayoutControl(),
                                ],
                            }),
                        ],
                    }),
                ],
            }),
        ],
    });
}

// --- Editor & viewer panes --------------------------------------------------

const editorInput: EdgyElement = InputEdgy({
    controller: editorCtrl,
    multiline: true,
    fontFamily: "monospace",
    fontSize: 13,
    color: tokens.text.code,
    cursorColor: tokens.accent.cursor,
    placeholderColor: tokens.text.placeholder,
    queryKey: ["editor-input"],
});

function Editor(): EdgyElement {
    return Container({
        color: tokens.bg.code,
        padding: 12,
        children: [
            // Rebuild the editor element whenever the selected case changes
            // so it re-reads the controller spans (reset by loadCase).
            Switch({
                value: selectedCase$,
                cases: CASE_NAMES.map((name) => ({
                    key: name,
                    child: editorInput,
                })),
                fallback: editorInput,
            }),
        ],
    });
}

function ReadyViewer(): EdgyElement {
    return Container({
        color: tokens.bg.viewer,
        padding: 12,
        children: [
            Switch({
                value: selectedCase$,
                cases: CASE_NAMES.map((name) => ({
                    key: name,
                    child: caseComponents.get(name)?.() ?? Placeholder(),
                })),
                fallback: Placeholder(),
            }),
        ],
    });
}

function ErrorPanel(): EdgyElement {
    return Container({
        color: tokens.bg.danger,
        alignment: 4, // Alignment.Center
        children: [
            Column({
                crossAlignment: CrossAxisAlignment.Center,
                children: [
                    Container({
                        width: 32,
                        height: 32,
                        borderRadius: 999,
                        color: tokens.status.error,
                        alignment: 4,
                        children: [
                            Text({
                                text: "!",
                                fontSize: 18,
                                color: tokens.text.inverse,
                            }),
                        ],
                    }),
                    SizedBox({ height: 16 }),
                    Text({
                        text: "Compile error",
                        fontSize: 14,
                        color: tokens.text.onDanger,
                    }),
                    SizedBox({ height: 8 }),
                    Text({
                        text: derive(() => get(errorMsg$)),
                        fontSize: 12,
                        color: tokens.text.onDanger,
                    }),
                ],
            }),
        ],
    });
}

function Viewer(): EdgyElement {
    return Switch({
        value: status$,
        cases: [
            { key: "ready", child: ReadyViewer() },
            { key: "error", child: ErrorPanel() },
        ],
        fallback: ReadyViewer(),
    });
}

function EditorAndViewer(): EdgyElement {
    return Row({
        children: [
            Expanded({
                flex: derive(() => layoutFlex("editor", get(layoutMode$))),
                child: Editor(),
            }),
            Expanded({
                flex: derive(() => layoutFlex("viewer", get(layoutMode$))),
                child: Viewer(),
            }),
        ],
    });
}

// --- Status bar -------------------------------------------------------------

function StatusDot(): EdgyElement {
    return Container({
        width: 6,
        height: 6,
        borderRadius: 999,
        color: derive(() =>
            get(status$) === "error"
                ? tokens.status.error
                : tokens.status.success,
        ),
    });
}

function StatusBar(): EdgyElement {
    return Container({
        color: tokens.bg.elevated,
        borderColor: tokens.border.subtle,
        borderWidth: 1,
        children: [
            Row({
                mainAlignment: MainAxisAlignment.SpaceBetween,
                children: [
                    // Left cluster: status dot + label, edited pill, timestamp.
                    Container({
                        padding: 4,
                        children: [
                            Row({
                                mainAxisSize: MainAxisSize.Min,
                                children: [
                                    SizedBox({ width: 8 }),
                                    StatusDot(),
                                    SizedBox({ width: 6 }),
                                    Text({
                                        text: derive(() => get(status$)),
                                        fontSize: 11,
                                        color: derive(() =>
                                            get(status$) === "error"
                                                ? tokens.status.error
                                                : tokens.status.success,
                                        ),
                                    }),
                                    // Edited indicator (only when edited).
                                    Condition({
                                        condition: edited$,
                                        child: Row({
                                            mainAxisSize: MainAxisSize.Min,
                                            children: [
                                                SizedBox({ width: 12 }),
                                                Container({
                                                    width: 6,
                                                    height: 6,
                                                    borderRadius: 999,
                                                    color: tokens.accent
                                                        .complement,
                                                }),
                                                SizedBox({ width: 6 }),
                                                Text({
                                                    text: "edited",
                                                    fontSize: 11,
                                                    color: tokens.text.tertiary,
                                                }),
                                            ],
                                        }),
                                        elseChild: SizedBox({ width: 0 }),
                                    }),
                                    SizedBox({ width: 12 }),
                                    Text({
                                        text: derive(
                                            () =>
                                                `compiled ${relativeTime(get(lastCompiledAtMs$), get(now$))}`,
                                        ),
                                        fontSize: 11,
                                        color: tokens.text.tertiary,
                                    }),
                                ],
                            }),
                        ],
                    }),
                    // Right cluster: keyboard hint + version.
                    Container({
                        padding: 4,
                        children: [
                            Row({
                                mainAxisSize: MainAxisSize.Min,
                                children: [
                                    Text({
                                        text: derive(() =>
                                            get(autoRun$)
                                                ? "auto-run on"
                                                : "\u2318S to run",
                                        ),
                                        fontSize: 11,
                                        color: tokens.text.tertiary,
                                    }),
                                    SizedBox({ width: 12 }),
                                    Text({
                                        text: "tur v0.1",
                                        fontSize: 11,
                                        color: tokens.text.tertiary,
                                    }),
                                    SizedBox({ width: 8 }),
                                ],
                            }),
                        ],
                    }),
                ],
            }),
        ],
    });
}

// ---------------------------------------------------------------------------
// Shell
// ---------------------------------------------------------------------------

const Shell: EdgyComponent = component(() =>
    Container({
        color: tokens.bg.app,
        children: [
            Column({
                crossAlignment: CrossAxisAlignment.Stretch,
                children: [
                    Toolbar(),
                    Expanded({
                        child: Row({
                            children: [
                                Sidebar(),
                                Expanded({ child: EditorAndViewer() }),
                            ],
                        }),
                    }),
                    StatusBar(),
                ],
            }),
        ],
    }),
);

render(Shell);
