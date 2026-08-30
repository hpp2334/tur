import {
    Column,
    Container,
    CrossAxisAlignment,
    derive,
    type Element,
    SizedBox,
    Switch,
    Text,
    VirtualAppView,
} from "tur:std";
import { app$, errorMsg$, status$ } from "../state";
import { tokens } from "../theme/tokens";

/** Painted while no controller is bound (or its child is still spawning) —
 *  reads the active controller's `status$` so the placeholder can say what
 *  it's waiting for. */
function FallbackView(): Element {
    return Container({
        color: tokens.bg.viewer,
        alignment: 4, // Alignment.Center
        children: [
            Text({
                text: derive((ctx) => {
                    const app = ctx.get(app$);
                    if (app == null) return "(no case)";
                    return ctx.get(app.status$) === "spawning"
                        ? "starting…"
                        : "(idle)";
                }),
                fontSize: 13,
                color: tokens.text.tertiary,
            }),
        ],
    });
}

/** Painted when the hosted child fails (module load / `start` errors) —
 *  reads the active controller's `errorMsg$`. */
function RuntimeErrorView(): Element {
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
                        text: "Runtime error",
                        fontSize: 14,
                        color: tokens.text.onDanger,
                    }),
                    SizedBox({ height: 8 }),
                    Text({
                        text: derive((ctx) => {
                            const app = ctx.get(app$);
                            return app == null ? "" : ctx.get(app.errorMsg$);
                        }),
                        fontSize: 12,
                        color: tokens.text.onDanger,
                    }),
                ],
            }),
        ],
    });
}

function ReadyViewer(): Element {
    return Container({
        color: tokens.bg.viewer,
        padding: 12,
        children: [
            // The selected case runs in a hosted child instance (own worker,
            // realm, store, tree). Binding the controller spawns the child
            // lazily; this element's own paint replays the child's frames.
            // Swapping the controller (case switch / recompile) unbinds the
            // old child (destroyed by `runCase`'s explicit `destroy$`) and
            // binds the new one. Viewer element churn (layout modes, mobile
            // tabs) is a keep-alive rebind — the child survives.
            VirtualAppView({
                app$,
                background: tokens.bg.viewer,
                fallback: FallbackView(),
                errorView: RuntimeErrorView(),
            }),
        ],
    });
}

function ErrorPanel(): Element {
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
                        text: derive((ctx) => ctx.get(errorMsg$)),
                        fontSize: 12,
                        color: tokens.text.onDanger,
                    }),
                ],
            }),
        ],
    });
}

export function Viewer(): Element {
    return Switch({
        value: status$,
        cases: [
            { key: "ready", child: ReadyViewer },
            { key: "error", child: ErrorPanel },
        ],
        fallback: ReadyViewer,
    });
}
