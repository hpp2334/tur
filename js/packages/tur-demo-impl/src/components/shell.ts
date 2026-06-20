import {
    Column,
    Container,
    CrossAxisAlignment,
    component,
    derive,
    type EdgyComponent,
    type EdgyElement,
    Expanded,
    get,
    Row,
    set,
} from "@tur/edgy";
import {
    editorFlex$,
    layoutFlex,
    layoutMode$,
    sidebarWidth$,
    viewerFlex$,
} from "../state";
import { tokens } from "../theme/tokens";
import { VDivider } from "./divider";
import { Editor } from "./editor";
import { Sidebar } from "./sidebar";
import { StatusBar } from "./status-bar";
import { Toolbar } from "./toolbar";
import { Viewer } from "./viewer";

/** Editor + viewer panes. The effective flex of each pane is the product of
 *  the layout-mode factor (`split`/`editor`/`viewer`) and the user-set drag
 *  weight. So when `layoutMode$` hides a pane (factor 0), it stays hidden
 *  regardless of the drag weight. */
function paneFlex(who: "editor" | "viewer"): EdgyElement {
    const baseFlex = who === "editor" ? editorFlex$ : viewerFlex$;
    return Expanded({
        flex: derive(() => {
            const modeFactor = layoutFlex(who, get(layoutMode$));
            return modeFactor * Math.max(0.0001, get(baseFlex));
        }),
        child: who === "editor" ? Editor() : Viewer(),
    });
}

function EditorAndViewer(): EdgyElement {
    return Row({
        crossAlignment: CrossAxisAlignment.Stretch,
        children: [
            paneFlex("editor"),
            VDivider({
                onDrag: (dx) => {
                    const cur = get(editorFlex$);
                    const other = get(viewerFlex$);
                    // Move the split proportionally to the drag delta: a full
                    // pane-width of drag shifts the split by ~50%. Clamp to
                    // keep both panes visible.
                    const total = cur + other;
                    if (total <= 0) return;
                    const sensitivity = 200;
                    const delta = dx / sensitivity;
                    let nextEditor = cur + delta * total;
                    let nextViewer = other - delta * total;
                    if (nextEditor < 0.1) {
                        nextEditor = 0.1;
                        nextViewer = total - 0.1;
                    }
                    if (nextViewer < 0.1) {
                        nextViewer = 0.1;
                        nextEditor = total - 0.1;
                    }
                    set(editorFlex$, nextEditor);
                    set(viewerFlex$, nextViewer);
                },
            }),
            paneFlex("viewer"),
        ],
    });
}

export const Shell: EdgyComponent = component(() =>
    Container({
        color: tokens.bg.app,
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
                                    onDrag: (dx) => {
                                        const next = Math.max(
                                            120,
                                            Math.min(
                                                480,
                                                get(sidebarWidth$) + dx,
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
        ],
    }),
);
