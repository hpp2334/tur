import {
    Condition,
    Container,
    derive,
    type Element,
    MainAxisAlignment,
    MainAxisSize,
    Row,
    SizedBox,
    Text,
} from "tur:std";
import {
    autoRun$,
    edited$,
    lastCompiledAtMs$,
    now$,
    relativeTime,
    status$,
} from "../state";
import { tokens } from "../theme/tokens";

function StatusDot(): Element {
    return Container()
        .width(6)
        .height(6)
        .borderRadius(999)
        .color(
            derive((ctx) =>
                ctx.get(status$) === "error"
                    ? tokens.status.error
                    : tokens.status.success,
            ),
        )
        .build();
}

export function StatusBar(): Element {
    return Container()
        .color(tokens.bg.elevated)
        .borderColor(tokens.border.subtle)
        .borderWidth(1)
        .children([
            Row()
                .mainAlignment(MainAxisAlignment.SpaceBetween)
                .children([
                    // Left cluster: status dot + label, edited pill, timestamp.
                    Container()
                        .padding(4)
                        .children([
                            Row()
                                .mainAxisSize(MainAxisSize.Min)
                                .children([
                                    SizedBox().width(8).build(),
                                    StatusDot(),
                                    SizedBox().width(6).build(),
                                    Text({
                                        text: derive((ctx) => ctx.get(status$)),
                                    })
                                        .fontSize(11)
                                        .color(
                                            derive((ctx) =>
                                                ctx.get(status$) === "error"
                                                    ? tokens.status.error
                                                    : tokens.status.success,
                                            ),
                                        )
                                        .build(),
                                    // Edited indicator (only when edited).
                                    Condition({ condition: edited$ })
                                        .elseChild(() =>
                                            SizedBox().width(0).build(),
                                        )
                                        .child(() =>
                                            Row()
                                                .mainAxisSize(MainAxisSize.Min)
                                                .children([
                                                    SizedBox()
                                                        .width(12)
                                                        .build(),
                                                    Container()
                                                        .width(6)
                                                        .height(6)
                                                        .borderRadius(999)
                                                        .color(
                                                            tokens.accent
                                                                .complement,
                                                        )
                                                        .build(),
                                                    SizedBox().width(6).build(),
                                                    Text({ text: "edited" })
                                                        .fontSize(11)
                                                        .color(
                                                            tokens.text
                                                                .tertiary,
                                                        )
                                                        .build(),
                                                ])
                                                .build(),
                                        )
                                        .build(),
                                    SizedBox().width(12).build(),
                                    Text({
                                        text: derive(
                                            (ctx) =>
                                                `compiled ${relativeTime(ctx.get(lastCompiledAtMs$), ctx.get(now$))}`,
                                        ),
                                    })
                                        .fontSize(11)
                                        .color(tokens.text.tertiary)
                                        .build(),
                                ])
                                .build(),
                        ])
                        .build(),
                    // Right cluster: keyboard hint + version.
                    Container()
                        .padding(4)
                        .children([
                            Row()
                                .mainAxisSize(MainAxisSize.Min)
                                .children([
                                    Text({
                                        text: derive((ctx) =>
                                            ctx.get(autoRun$)
                                                ? "auto-run on"
                                                : "\u2318S to run",
                                        ),
                                    })
                                        .fontSize(11)
                                        .color(tokens.text.tertiary)
                                        .build(),
                                    SizedBox().width(12).build(),
                                    Text({ text: "tur v0.1" })
                                        .fontSize(11)
                                        .color(tokens.text.tertiary)
                                        .build(),
                                    SizedBox().width(8).build(),
                                ])
                                .build(),
                        ])
                        .build(),
                ])
                .build(),
        ])
        .build();
}
