import {
    Condition,
    Container,
    derive,
    type Element,
    Expanded,
    Image,
    MainAxisSize,
    MouseRegion,
    mutate,
    PointerInteract,
    Positioned,
    Row,
    SizedBox,
    Stack,
    Text,
} from "tur:std";
import {
    autoRun$,
    isMobile$,
    type LayoutMode,
    layoutHovered$,
    layoutMode$,
    recompile,
    resetCase,
    resetHovered$,
    runHovered$,
    selectedCase$,
} from "../state";
import { tokens } from "../theme/tokens";
import { resetIconId, runIconId } from "./icons";

// --- Run button ------------------------------------------------------------

function RunButton(): Element {
    return MouseRegion()
        .cursor("pointer")
        .onEnter(mutate((ctx, _ev) => ctx.set(runHovered$, true)))
        .onExit(mutate((ctx, _ev) => ctx.set(runHovered$, false)))
        .child(
            PointerInteract()
                .onClick(mutate((_ctx, _ev) => _ctx.set(recompile)))
                .child(
                    Container()
                        .padding(6)
                        .borderRadius(6)
                        .color(
                            derive((ctx) =>
                                ctx.get(runHovered$)
                                    ? tokens.bg.button.primaryHover
                                    : tokens.bg.button.primary,
                            ),
                        )
                        .children([
                            Row()
                                .mainAxisSize(MainAxisSize.Min)
                                .children([
                                    Image({ resourceId: runIconId })
                                        .width(10)
                                        .height(10)
                                        .build(),
                                    SizedBox().width(4).build(),
                                    Text({ text: "Run" })
                                        .fontSize(12)
                                        .color(tokens.text.onAccent)
                                        .build(),
                                ])
                                .build(),
                        ])
                        .build(),
                )
                .build(),
        )
        .build();
}

// --- Reset button ----------------------------------------------------------

function ResetButton(): Element {
    return MouseRegion()
        .cursor("pointer")
        .onEnter(mutate((ctx, _ev) => ctx.set(resetHovered$, true)))
        .onExit(mutate((ctx, _ev) => ctx.set(resetHovered$, false)))
        .child(
            PointerInteract()
                .onClick(mutate((_ctx, _ev) => _ctx.set(resetCase)))
                .child(
                    Container()
                        .padding(6)
                        .borderRadius(6)
                        .color(
                            derive((ctx) =>
                                ctx.get(resetHovered$)
                                    ? tokens.bg.hover
                                    : tokens.bg.button.ghost,
                            ),
                        )
                        .children([
                            Row()
                                .mainAxisSize(MainAxisSize.Min)
                                .children([
                                    Image({ resourceId: resetIconId })
                                        .width(12)
                                        .height(12)
                                        .build(),
                                    SizedBox().width(4).build(),
                                    Text({ text: "Reset" })
                                        .fontSize(12)
                                        .color(tokens.text.secondary)
                                        .build(),
                                ])
                                .build(),
                        ])
                        .build(),
                )
                .build(),
        )
        .build();
}

// --- Auto-run toggle -------------------------------------------------------

function AutoRunToggle(): Element {
    return Row()
        .mainAxisSize(MainAxisSize.Min)
        .children([
            Text({ text: "auto" })
                .fontSize(11)
                .color(tokens.text.secondary)
                .build(),
            SizedBox().width(6).build(),
            MouseRegion()
                .cursor("pointer")
                .child(
                    PointerInteract()
                        .onClick(
                            mutate((ctx, _ev) =>
                                ctx.set(autoRun$, !ctx.get(autoRun$)),
                            ),
                        )
                        .child(
                            Container()
                                .width(28)
                                .height(16)
                                .borderRadius(999)
                                .color(
                                    derive((ctx) =>
                                        ctx.get(autoRun$)
                                            ? tokens.bg.button.primary
                                            : tokens.bg.hover,
                                    ),
                                )
                                .children([
                                    Stack()
                                        .children([
                                            Positioned()
                                                .top(2)
                                                .left(
                                                    derive((ctx) =>
                                                        ctx.get(autoRun$)
                                                            ? 14
                                                            : 2,
                                                    ),
                                                )
                                                .child(
                                                    Container()
                                                        .width(12)
                                                        .height(12)
                                                        .borderRadius(999)
                                                        .color(
                                                            tokens.text.inverse,
                                                        )
                                                        .build(),
                                                )
                                                .build(),
                                        ])
                                        .build(),
                                ])
                                .build(),
                        )
                        .build(),
                )
                .build(),
        ])
        .build();
}

// --- Layout mode segmented control -----------------------------------------

function LayoutButton(mode: LayoutMode, label: string): Element {
    return MouseRegion()
        .cursor("pointer")
        .onEnter(mutate((ctx, _ev) => ctx.set(layoutHovered$, mode)))
        .onExit(mutate((ctx, _ev) => ctx.set(layoutHovered$, null)))
        .child(
            PointerInteract()
                .onClick(mutate((ctx, _ev) => ctx.set(layoutMode$, mode)))
                .child(
                    Container()
                        .padding(6)
                        .color(
                            derive((ctx) => {
                                const selected = ctx.get(layoutMode$) === mode;
                                const hovered =
                                    ctx.get(layoutHovered$) === mode;
                                if (selected) return tokens.bg.controlSelected;
                                if (hovered) return tokens.bg.controlTrayHover;
                                return tokens.bg.controlTray;
                            }),
                        )
                        .children([
                            Text({ text: label })
                                .fontSize(11)
                                .color(
                                    derive((ctx) =>
                                        ctx.get(layoutMode$) === mode
                                            ? tokens.text.primary
                                            : tokens.text.secondary,
                                    ),
                                )
                                .build(),
                        ])
                        .build(),
                )
                .build(),
        )
        .build();
}

function LayoutControl(): Element {
    return Container()
        .color(tokens.bg.controlTray)
        .children([
            Row()
                .mainAxisSize(MainAxisSize.Min)
                .children([
                    LayoutButton("split", "Split"),
                    LayoutButton("editor", "Edit"),
                    LayoutButton("viewer", "View"),
                ])
                .build(),
        ])
        .build();
}

// --- Toolbar (composite) ---------------------------------------------------

export function Toolbar(): Element {
    return Container()
        .color(tokens.bg.elevated)
        .borderColor(tokens.border.subtle)
        .borderWidth(1)
        .children([
            Row()
                .children([
                    // Brand.
                    Container()
                        .padding(derive((ctx) => (ctx.get(isMobile$) ? 8 : 12)))
                        .children([
                            Row()
                                .mainAxisSize(MainAxisSize.Min)
                                .children([
                                    Text({ text: "tur" })
                                        .fontSize(14)
                                        .color(tokens.accent.solid)
                                        .build(),
                                    // "playground" subtitle — hidden on mobile
                                    // to reclaim horizontal space.
                                    Condition({
                                        condition: derive(
                                            (ctx) => !ctx.get(isMobile$),
                                        ),
                                    })
                                        .elseChild(() =>
                                            SizedBox().width(0).build(),
                                        )
                                        .child(() =>
                                            Row()
                                                .mainAxisSize(MainAxisSize.Min)
                                                .children([
                                                    SizedBox().width(4).build(),
                                                    Text({ text: "playground" })
                                                        .fontSize(11)
                                                        .color(
                                                            tokens.text
                                                                .secondary,
                                                        )
                                                        .build(),
                                                ])
                                                .build(),
                                        )
                                        .build(),
                                ])
                                .build(),
                        ])
                        .build(),
                    // Case name (center, expands).
                    Expanded()
                        .child(
                            Container()
                                .padding(
                                    derive((ctx) =>
                                        ctx.get(isMobile$) ? 8 : 12,
                                    ),
                                )
                                .children([
                                    Text({
                                        text: derive((ctx) =>
                                            ctx.get(selectedCase$),
                                        ),
                                    })
                                        .fontSize(13)
                                        .color(tokens.text.body)
                                        .build(),
                                ])
                                .build(),
                        )
                        .build(),
                    // Actions.
                    Container()
                        .padding(derive((ctx) => (ctx.get(isMobile$) ? 8 : 12)))
                        .children([
                            Row()
                                .mainAxisSize(MainAxisSize.Min)
                                .children([
                                    RunButton(),
                                    SizedBox().width(6).build(),
                                    ResetButton(),
                                    SizedBox().width(12).build(),
                                    AutoRunToggle(),
                                    // Layout segmented control — desktop only
                                    // (mobile uses the bottom tab bar).
                                    Condition({
                                        condition: derive(
                                            (ctx) => !ctx.get(isMobile$),
                                        ),
                                    })
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
                                                    LayoutControl(),
                                                ])
                                                .build(),
                                        )
                                        .build(),
                                ])
                                .build(),
                        ])
                        .build(),
                ])
                .build(),
        ])
        .build();
}
