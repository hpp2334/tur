import {
    Color,
    Column,
    Container,
    CrossAxisAlignment,
    component,
    createTextEditingController,
    Dynamic,
    derive,
    type EdgyElement,
    Expanded,
    get,
    InputEdgy,
    type KeyEvent,
    MainAxisAlignment,
    Match,
    mutate,
    PointerInteract,
    Row,
    render,
    ScrollView,
    SizedBox,
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
const CASE_NAMES = Object.keys(CASE_SOURCES).sort();
const INITIAL_CASE = CASE_NAMES.includes("counter")
    ? "counter"
    : (CASE_NAMES[0] ?? "");

const selectedCase$ = source<string>(INITIAL_CASE);
const status$ = source<string>("ready");
const errorMsg$ = source("");
// Atom holding the element currently rendered in the viewer.
const currentCaseElement$ = source<EdgyElement>(Placeholder());

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
// Case loading / compilation
// ---------------------------------------------------------------------------
function loadCase(name: string): void {
    const src = CASE_SOURCES[name];
    if (src === undefined) return;
    set(selectedCase$, name);
    editorCtrl.setSpans(buildHighlightSpans(src));
    const result = compileCase(src);
    if (result.error || !result.component) {
        set(status$, "error");
        set(errorMsg$, result.error ?? "unknown error");
        return;
    }
    set(status$, "ready");
    set(errorMsg$, "");
    set(currentCaseElement$, (result.component as () => EdgyElement)());
}

function recompile(): void {
    const src = editorCtrl.text;
    const result = compileCase(src);
    if (result.error || !result.component) {
        set(status$, "error");
        set(errorMsg$, result.error ?? "unknown error");
        return;
    }
    set(status$, "ready");
    set(errorMsg$, "");
    set(currentCaseElement$, (result.component as () => EdgyElement)());
}

// Load the initial case once the bundle has evaluated.
loadCase(INITIAL_CASE);

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
                                Dynamic({
                                    child: derive(() => {
                                        // Rebuild the editor element when the selected case
                                        // changes (loadCase resets the controller spans first).
                                        void get(selectedCase$);
                                        return InputEdgy({
                                            controller: editorCtrl,
                                            multiline: true,
                                            fontFamily: "monospace",
                                            fontSize: 13,
                                            color: Color.hex("#cdd6f4"),
                                            cursorColor: Color.hex("#f5e0dc"),
                                            placeholderColor:
                                                Color.hex("#585b70"),
                                        });
                                    }),
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
                                    Match({
                                        value: status$,
                                        cases: [
                                            [
                                                "ready",
                                                Text({
                                                    text: "ready",
                                                    fontSize: 11,
                                                    color: Color.hex("#22c55e"),
                                                }),
                                            ],
                                            [
                                                "error",
                                                Text({
                                                    text: "error",
                                                    fontSize: 11,
                                                    color: Color.hex("#ef4444"),
                                                }),
                                            ],
                                        ],
                                    }),
                                ],
                            }),
                        ],
                    }),
                    Expanded({
                        child: Dynamic({ child: currentCaseElement$ }),
                    }),
                    // Error overlay.
                    Dynamic({
                        child: derive(() => {
                            const msg = get(errorMsg$);
                            if (!msg) return SizedBox({ height: 0 });
                            return Container({
                                padding: 8,
                                color: Color.hex("#3b1116"),
                                children: [
                                    Text({
                                        text: msg,
                                        fontSize: 11,
                                        color: Color.hex("#fca5a5"),
                                    }),
                                ],
                            });
                        }),
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
