import {
    Column,
    Condition,
    Container,
    CrossAxisAlignment,
    derive,
    type Element,
    Expanded,
    mutate,
    Row,
    Stack,
    Switch,
    view,
} from "tur:std";
import {
    editorWidth$,
    isMobile$,
    layoutMode$,
    mobileTab$,
    sidebarWidth$,
} from "../state";
import { tokens } from "../theme/tokens";
import { ContextMenuOverlay } from "./context-menu";
import { VDivider } from "./divider";
import { Editor } from "./editor";
import { MobileTabBar } from "./mobile-tab-bar";
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
                value: derive((ctx) => ctx.get(layoutMode$)),
                cases: [
                    {
                        key: "split",
                        child: () =>
                            Container({
                                width: derive((ctx) => ctx.get(editorWidth$)),
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
                condition: derive((ctx) => ctx.get(layoutMode$) === "split"),
                child: () =>
                    VDivider({
                        onDrag: mutate((ctx, ev) => {
                            const next = Math.max(
                                100,
                                ctx.get(editorWidth$) + ev.deltaFromLast.x,
                            );
                            ctx.set(editorWidth$, next);
                        }),
                    }),
            }),
            // Viewer pane
            Switch({
                value: derive((ctx) => ctx.get(layoutMode$)),
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
                    Condition({
                        condition: isMobile$,
                        // Mobile: one full-width pane at a time, switched via
                        // the bottom tab bar. No dividers — each pane fills.
                        child: () =>
                            Column({
                                crossAlignment: CrossAxisAlignment.Stretch,
                                children: [
                                    Toolbar(),
                                    Expanded({
                                        child: Switch({
                                            value: derive((ctx) =>
                                                ctx.get(mobileTab$),
                                            ),
                                            cases: [
                                                {
                                                    key: "cases",
                                                    child: () => Sidebar(),
                                                },
                                                {
                                                    key: "edit",
                                                    child: () => Editor(),
                                                },
                                            ],
                                            fallback: () => Viewer(),
                                        }),
                                    }),
                                    MobileTabBar(),
                                    StatusBar(),
                                ],
                            }),
                        // Desktop: 3-pane layout (sidebar | editor | viewer)
                        // with draggable dividers.
                        elseChild: () =>
                            Column({
                                crossAlignment: CrossAxisAlignment.Stretch,
                                children: [
                                    Toolbar(),
                                    Expanded({
                                        child: Row({
                                            crossAlignment:
                                                CrossAxisAlignment.Stretch,
                                            children: [
                                                Sidebar(),
                                                VDivider({
                                                    onDrag: mutate(
                                                        (ctx, ev) => {
                                                            const next =
                                                                Math.max(
                                                                    120,
                                                                    Math.min(
                                                                        480,
                                                                        ctx.get(
                                                                            sidebarWidth$,
                                                                        ) +
                                                                            ev
                                                                                .deltaFromLast
                                                                                .x,
                                                                    ),
                                                                );
                                                            ctx.set(
                                                                sidebarWidth$,
                                                                next,
                                                            );
                                                        },
                                                    ),
                                                }),
                                                Expanded({
                                                    child: EditorAndViewer(),
                                                }),
                                            ],
                                        }),
                                    }),
                                    StatusBar(),
                                ],
                            }),
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
