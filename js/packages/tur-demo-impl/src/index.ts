import {
    Color,
    Column,
    Condition,
    Container,
    CrossAxisAlignment,
    component,
    createTextEditingController,
    derive,
    type EdgyElement,
    Expanded,
    get,
    InputEdgy,
    type KeyEvent,
    MainAxisAlignment,
    mutate,
    PointerInteract,
    Row,
    render,
    ScrollView,
    SizedBox,
    Switch,
    set,
    source,
    Text,
} from "@tur/edgy";
import { CASE_SOURCES } from "./cases-generated";
import { buildHighlightSpans, compileCase } from "./compile";

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
// Reactive state
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

const selectedCase$ = source<string>(INITIAL_CASE);
const status$ = source<string>("ready");
const errorMsg$ = source("");

// One compiled component factory per case. Pre-compiled at startup so the
// viewer can declare a `Switch` with a case per name. Recompile (Cmd-S)
// overwrites an entry and re-renders the shell so the new component is used.
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

// The editor controller. `onInput` re-highlights live; `onKeyDown` handles
// Cmd-S to recompile.
const editorCtrl = createTextEditingController({
    onInput: mutate((_ctx, _text: string, _enter: boolean) => {
        editorCtrl.setSpansPreserveCursor(buildHighlightSpans(editorCtrl.text));
    }),
    onKeyDown: mutate((_ctx, ev: KeyEvent) => {
        if (ev.key === "s" && (ev.meta || ev.ctrl)) {
            recompile();
        }
    }),
}) as unknown as EditorController;

// ---------------------------------------------------------------------------
// Case loading / recompilation
// ---------------------------------------------------------------------------
function loadCase(name: string): void {
    if (!CASE_SOURCES[name]) return;
    set(selectedCase$, name);
    editorCtrl.setSpans(buildHighlightSpans(CASE_SOURCES[name]));
    set(status$, "ready");
    set(errorMsg$, "");
}

function recompile(): void {
    const name = get(selectedCase$);
    const result = compileCase(editorCtrl.text);
    if (result.error || !result.component) {
        set(status$, "error");
        set(errorMsg$, result.error ?? "unknown error");
        return;
    }
    caseComponents.set(name, result.component as () => EdgyElement);
    set(status$, "ready");
    set(errorMsg$, "");
    // Rebuild the shell so the viewer's Switch picks up the new component.
    render(Shell);
}

// Initialise the editor with the first case's source.
editorCtrl.setSpans(buildHighlightSpans(CASE_SOURCES[INITIAL_CASE] ?? ""));

// ---------------------------------------------------------------------------
// UI primitives
// ---------------------------------------------------------------------------
function Placeholder(): EdgyElement {
    return Container({
        color: Color.hex("#0f172a"),
        alignment: 4, // Alignment.Center
        children: [
            Text({
                text: "(no case)",
                fontSize: 14,
                color: Color.hex("#475569"),
            }),
        ],
    });
}

function Sidebar(): EdgyElement {
    const items: EdgyElement[] = CASE_NAMES.map((name) =>
        PointerInteract({
            onClick: mutate(() => loadCase(name)),
            child: Container({
                padding: 8,
                color: derive(() =>
                    get(selectedCase$) === name
                        ? Color.hex("#1e293b")
                        : Color.hex("#0f172a"),
                ),
                children: [
                    Text({
                        text: name,
                        fontSize: 12,
                        color: derive(() =>
                            get(selectedCase$) === name
                                ? Color.hex("#e2e8f0")
                                : Color.hex("#94a3b8"),
                        ),
                    }),
                ],
            }),
        }),
    );

    return Container({
        width: 220,
        color: Color.hex("#0f172a"),
        children: [
            Column({
                children: [
                    Container({
                        padding: 12,
                        children: [
                            Text({
                                text: "tur playground",
                                fontSize: 14,
                                color: Color.hex("#e2e8f0"),
                            }),
                        ],
                    }),
                    Expanded({
                        child: ScrollView({
                            child: Column({
                                crossAlignment: CrossAxisAlignment.Start,
                                children: items,
                            }),
                        }),
                    }),
                ],
            }),
        ],
    });
}

// A single editor input spec, reused as the child of every editor Switch
// case. Switch rebuilds it on case change, which forces the EditableText to
// re-read the (just-updated) controller spans.
const editorInput: EdgyElement = InputEdgy({
    controller: editorCtrl,
    multiline: true,
    fontFamily: "monospace",
    fontSize: 13,
    color: Color.hex("#cdd6f4"),
    cursorColor: Color.hex("#f5e0dc"),
    placeholderColor: Color.hex("#585b70"),
});

function Editor(): EdgyElement {
    return Container({
        color: Color.hex("#1e1e2e"),
        children: [
            Column({
                crossAlignment: CrossAxisAlignment.Stretch,
                children: [
                    // Header: case name + run hint.
                    Container({
                        padding: 8,
                        color: Color.hex("#11111b"),
                        children: [
                            Row({
                                mainAlignment: MainAxisAlignment.SpaceBetween,
                                children: [
                                    Text({
                                        text: derive(
                                            () =>
                                                `editor — ${get(selectedCase$)}`,
                                        ),
                                        fontSize: 12,
                                        color: Color.hex("#94a3b8"),
                                    }),
                                    PointerInteract({
                                        onClick: mutate(() => recompile()),
                                        child: Container({
                                            padding: 6,
                                            color: Color.hex("#313244"),
                                            children: [
                                                Text({
                                                    text: "Run (Cmd-S)",
                                                    fontSize: 11,
                                                    color: Color.hex("#cdd6f4"),
                                                }),
                                            ],
                                        }),
                                    }),
                                ],
                            }),
                        ],
                    }),
                    Expanded({
                        child: Container({
                            padding: 8,
                            children: [
                                // Rebuild the editor element whenever the
                                // selected case changes so it re-reads the
                                // controller spans (reset by loadCase).
                                Switch({
                                    value: selectedCase$,
                                    cases: CASE_NAMES.map((name) => ({
                                        key: name,
                                        child: editorInput,
                                    })),
                                    fallback: editorInput,
                                }),
                            ],
                        }),
                    }),
                ],
            }),
        ],
    });
}

function Viewer(): EdgyElement {
    return Container({
        color: Color.hex("#0b0b13"),
        children: [
            Column({
                crossAlignment: CrossAxisAlignment.Stretch,
                children: [
                    Container({
                        padding: 8,
                        color: Color.hex("#11111b"),
                        children: [
                            Row({
                                mainAlignment: MainAxisAlignment.SpaceBetween,
                                children: [
                                    Text({
                                        text: "viewer",
                                        fontSize: 12,
                                        color: Color.hex("#94a3b8"),
                                    }),
                                    Switch({
                                        value: status$,
                                        cases: [
                                            {
                                                key: "ready",
                                                child: Text({
                                                    text: "ready",
                                                    fontSize: 11,
                                                    color: Color.hex("#22c55e"),
                                                }),
                                            },
                                            {
                                                key: "error",
                                                child: Text({
                                                    text: "error",
                                                    fontSize: 11,
                                                    color: Color.hex("#ef4444"),
                                                }),
                                            },
                                        ],
                                    }),
                                ],
                            }),
                        ],
                    }),
                    Expanded({
                        child: Switch({
                            value: selectedCase$,
                            cases: CASE_NAMES.map((name) => ({
                                key: name,
                                child:
                                    caseComponents.get(name)?.() ??
                                    Placeholder(),
                            })),
                            fallback: Placeholder(),
                        }),
                    }),
                    // Error overlay.
                    Condition({
                        condition: derive(() => !!get(errorMsg$)),
                        child: Container({
                            padding: 8,
                            color: Color.hex("#3b1116"),
                            children: [
                                Text({
                                    text: derive(() => get(errorMsg$)),
                                    fontSize: 11,
                                    color: Color.hex("#fca5a5"),
                                }),
                            ],
                        }),
                        elseChild: SizedBox({ height: 0 }),
                    }),
                ],
            }),
        ],
    });
}

const Shell = component(() =>
    Expanded({
        child: Container({
            color: Color.hex("#0f172a"),
            children: [
                Row({
                    children: [
                        Sidebar(),
                        Expanded({ child: Editor() }),
                        Expanded({ child: Viewer() }),
                    ],
                }),
            ],
        }),
    }),
);

render(Shell);
