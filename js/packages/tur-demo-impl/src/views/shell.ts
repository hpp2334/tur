import {
    Column,
    Condition,
    Container,
    CrossAxisAlignment,
    derive,
    type Element,
    Expanded,
    get,
    Row,
    Stack,
    Switch,
    set,
    view,
} from "builtin:tur/std";
import { editorWidth$, layoutMode$, sidebarWidth$ } from "../state";
import { tokens } from "../theme/tokens";
import { ContextMenuOverlay } from "./context-menu";
import { VDivider } from "./divider";
import { Editor } from "./editor";
import { Sidebar } from "./sidebar";
import { StatusBar } from "./status-bar";
import { Toolbar } from "./toolbar";
import { Viewer } from "./viewer";

/** Editor + viewer panes. The editor uses a fixed pixel width (dragged 1:1
 *  with the mouse); the viewer is `Expanded` and fills the rest. Layout modes
 *  swap which pane is visible:
 *  - `split`: editor = fixed `editorWidth$`, viewer = Expanded.
 *  - `editor`: editor = Expanded (fills all), viewer hidden.
 *  - `viewer`: editor hidden, viewer = Expanded. */
function EditorAndViewer(): Element {
    return Row({
        crossAlignment: CrossAxisAlignment.Stretch,
        children: [
            // Editor pane
            Switch({
                value: derive(() => get(layoutMode$)),
                cases: [
                    {
                        key: "split",
                        child: () =>
                            Container({
                                width: derive(() => get(editorWidth$)),
                                children: [Editor()],
                            }),
                    },
                    {
                        key: "editor",
                        child: () => Expanded({ child: Editor() }),
                    },
                ],
                fallback: () => Container({ width: 0, children: [Editor()] }),
            }),
            // Divider — only in split mode
            Condition({
                condition: derive(() => get(layoutMode$) === "split"),
                child: () =>
                    VDivider({
                        onDrag: (ev) => {
                            const next = Math.max(
                                100,
                                get(editorWidth$) + ev.deltaFromLast.x,
                            );
                            set(editorWidth$, next);
                        },
                    }),
            }),
            // Viewer pane
            Switch({
                value: derive(() => get(layoutMode$)),
                cases: [
                    {
                        key: "split",
                        child: () => Expanded({ child: Viewer() }),
                    },
                    {
                        key: "viewer",
                        child: () => Expanded({ child: Viewer() }),
                    },
                ],
                fallback: () => Container({ width: 0, children: [Viewer()] }),
            }),
        ],
    });
}

export const Shell: Element = view(() =>
    Container({
        color: tokens.bg.app,
        children: [
            Stack({
                children: [
                    Column({
                        crossAlignment: CrossAxisAlignment.Stretch,
                        children: [
                            Toolbar(),
                            Expanded({
                                child: Row({
                                    crossAlignment: CrossAxisAlignment.Stretch,
                                    children: [
                                        Sidebar(),
                                        VDivider({
                                            onDrag: (ev) => {
                                                const next = Math.max(
                                                    120,
                                                    Math.min(
                                                        480,
                                                        get(sidebarWidth$) +
                                                            ev.deltaFromLast.x,
                                                    ),
                                                );
                                                set(sidebarWidth$, next);
                                            },
                                        }),
                                        Expanded({ child: EditorAndViewer() }),
                                    ],
                                }),
                            }),
                            StatusBar(),
                        ],
                    }),
                    // Context-menu overlay — paints on top of everything
                    // when open. Lives at the canvas root so it can be
                    // positioned at any canvas-relative coord.
                    ContextMenuOverlay(),
                ],
            }),
        ],
    }),
);
