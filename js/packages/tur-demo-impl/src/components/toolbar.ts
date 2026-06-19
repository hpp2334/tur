import {
    Container,
    derive,
    type EdgyElement,
    Expanded,
    get,
    MainAxisSize,
    mutate,
    PointerInteract,
    Positioned,
    Row,
    SizedBox,
    Stack,
    set,
    Text,
} from "@tur/edgy";
import {
    autoRun$,
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

// --- Run button ------------------------------------------------------------

function RunButton(): EdgyElement {
    return PointerInteract({
        onClick: mutate(recompile),
        onPointerEnter: mutate(() => set(runHovered$, true)),
        onPointerExit: mutate(() => set(runHovered$, false)),
        child: Container({
            padding: 6,
            borderRadius: 6,
            color: derive(() =>
                get(runHovered$)
                    ? tokens.bg.button.primaryHover
                    : tokens.bg.button.primary,
            ),
            children: [
                Row({
                    mainAxisSize: MainAxisSize.Min,
                    children: [
                        Text({
                            text: "\u25B6", // ▶
                            fontSize: 9,
                            color: tokens.text.onAccent,
                        }),
                        SizedBox({ width: 4 }),
                        Text({
                            text: "Run",
                            fontSize: 12,
                            color: tokens.text.onAccent,
                        }),
                    ],
                }),
            ],
        }),
    });
}

// --- Reset button ----------------------------------------------------------

function ResetButton(): EdgyElement {
    return PointerInteract({
        onClick: mutate(resetCase),
        onPointerEnter: mutate(() => set(resetHovered$, true)),
        onPointerExit: mutate(() => set(resetHovered$, false)),
        child: Container({
            padding: 6,
            borderRadius: 6,
            color: derive(() =>
                get(resetHovered$) ? tokens.bg.hover : tokens.bg.button.ghost,
            ),
            children: [
                Row({
                    mainAxisSize: MainAxisSize.Min,
                    children: [
                        Text({
                            text: "\u21BA", // ↺
                            fontSize: 12,
                            color: tokens.text.secondary,
                        }),
                        SizedBox({ width: 4 }),
                        Text({
                            text: "Reset",
                            fontSize: 12,
                            color: tokens.text.secondary,
                        }),
                    ],
                }),
            ],
        }),
    });
}

// --- Auto-run toggle -------------------------------------------------------

function AutoRunToggle(): EdgyElement {
    return Row({
        mainAxisSize: MainAxisSize.Min,
        children: [
            Text({
                text: "auto",
                fontSize: 11,
                color: tokens.text.secondary,
            }),
            SizedBox({ width: 6 }),
            PointerInteract({
                onClick: mutate(() => set(autoRun$, !get(autoRun$))),
                child: Container({
                    width: 28,
                    height: 16,
                    borderRadius: 999,
                    color: derive(() =>
                        get(autoRun$)
                            ? tokens.bg.button.primary
                            : tokens.bg.hover,
                    ),
                    children: [
                        Stack({
                            children: [
                                Positioned({
                                    top: 2,
                                    left: derive(() =>
                                        get(autoRun$) ? 14 : 2,
                                    ),
                                    child: Container({
                                        width: 12,
                                        height: 12,
                                        borderRadius: 999,
                                        color: tokens.text.inverse,
                                    }),
                                }),
                            ],
                        }),
                    ],
                }),
            }),
        ],
    });
}

// --- Layout mode segmented control -----------------------------------------

function LayoutButton(mode: LayoutMode, label: string): EdgyElement {
    return PointerInteract({
        onClick: mutate(() => set(layoutMode$, mode)),
        onPointerEnter: mutate(() => set(layoutHovered$, mode)),
        onPointerExit: mutate(() => set(layoutHovered$, null)),
        child: Container({
            padding: 6,
            color: derive(() => {
                const selected = get(layoutMode$) === mode;
                const hovered = get(layoutHovered$) === mode;
                if (selected) return tokens.bg.controlSelected;
                if (hovered) return tokens.bg.controlTrayHover;
                return tokens.bg.controlTray;
            }),
            children: [
                Text({
                    text: label,
                    fontSize: 11,
                    color: derive(() =>
                        get(layoutMode$) === mode
                            ? tokens.text.primary
                            : tokens.text.secondary,
                    ),
                }),
            ],
        }),
    });
}

function LayoutControl(): EdgyElement {
    return Container({
        color: tokens.bg.controlTray,
        children: [
            Row({
                mainAxisSize: MainAxisSize.Min,
                children: [
                    LayoutButton("split", "Split"),
                    LayoutButton("editor", "Edit"),
                    LayoutButton("viewer", "View"),
                ],
            }),
        ],
    });
}

// --- Toolbar (composite) ---------------------------------------------------

export function Toolbar(): EdgyElement {
    return Container({
        color: tokens.bg.elevated,
        borderColor: tokens.border.subtle,
        borderWidth: 1,
        children: [
            Row({
                children: [
                    // Brand.
                    Container({
                        padding: 12,
                        children: [
                            Row({
                                mainAxisSize: MainAxisSize.Min,
                                children: [
                                    Text({
                                        text: "tur",
                                        fontSize: 14,
                                        color: tokens.accent.solid,
                                    }),
                                    SizedBox({ width: 4 }),
                                    Text({
                                        text: "playground",
                                        fontSize: 11,
                                        color: tokens.text.secondary,
                                    }),
                                ],
                            }),
                        ],
                    }),
                    // Case name (center, expands).
                    Expanded({
                        child: Container({
                            padding: 12,
                            children: [
                                Text({
                                    text: derive(() => get(selectedCase$)),
                                    fontSize: 13,
                                    color: tokens.text.body,
                                }),
                            ],
                        }),
                    }),
                    // Actions.
                    Container({
                        padding: 12,
                        children: [
                            Row({
                                mainAxisSize: MainAxisSize.Min,
                                children: [
                                    RunButton(),
                                    SizedBox({ width: 6 }),
                                    ResetButton(),
                                    SizedBox({ width: 12 }),
                                    AutoRunToggle(),
                                    SizedBox({ width: 12 }),
                                    LayoutControl(),
                                ],
                            }),
                        ],
                    }),
                ],
            }),
        ],
    });
}
