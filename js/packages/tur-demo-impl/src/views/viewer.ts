import {
    Column,
    Container,
    CrossAxisAlignment,
    derive,
    Each,
    type Element,
    get,
    SizedBox,
    Switch,
    Text,
} from "builtin:tur/std";
import {
    compileVersion$,
    errorMsg$,
    getCaseView,
    selectedCase$,
    status$,
} from "../state";
import { FadeIn } from "../state/transitions";
import { tokens } from "../theme/tokens";

function Placeholder(): Element {
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

function ReadyViewer(): Element {
    return Container({
        color: tokens.bg.viewer,
        padding: 12,
        children: [
            // Rebuild the rendered case view whenever the selected case
            // changes OR a fresh compile lands (compileVersion$ bumps on each
            // successful recompile). Each rebuilds its children when the items
            // array identity changes — Switch can't do this because its case
            // keys are static.
            Each({
                items: derive(() => [
                    {
                        name: get(selectedCase$),
                        v: get(compileVersion$),
                    },
                ]),
                build: (item) =>
                    FadeIn({
                        child: getCaseView(item.name) ?? Placeholder(),
                    }),
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
                        text: derive(() => get(errorMsg$)),
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
