import {
    Column,
    Container,
    CrossAxisAlignment,
    derive,
    type EdgyElement,
    get,
    SizedBox,
    Switch,
    Text,
} from "@tur/edgy";
import {
    CASE_NAMES,
    errorMsg$,
    getCaseComponent,
    selectedCase$,
    status$,
} from "../state";
import { tokens } from "../theme/tokens";

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

function ReadyViewer(): EdgyElement {
    return Container({
        color: tokens.bg.viewer,
        padding: 12,
        children: [
            Switch({
                value: selectedCase$,
                cases: CASE_NAMES.map((name) => ({
                    key: name,
                    child: () => getCaseComponent(name) ?? Placeholder(),
                })),
                fallback: () => Placeholder(),
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

export function Viewer(): EdgyElement {
    return Switch({
        value: status$,
        cases: [
            { key: "ready", child: ReadyViewer },
            { key: "error", child: ErrorPanel },
        ],
        fallback: ReadyViewer,
    });
}
