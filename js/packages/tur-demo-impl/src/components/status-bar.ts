import {
    Condition,
    Container,
    derive,
    type EdgyElement,
    get,
    MainAxisAlignment,
    MainAxisSize,
    Row,
    SizedBox,
    Text,
} from "@tur/edgy";
import {
    autoRun$,
    edited$,
    lastCompiledAtMs$,
    now$,
    relativeTime,
    status$,
} from "../state";
import { tokens } from "../theme/tokens";

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

export function StatusBar(): EdgyElement {
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
