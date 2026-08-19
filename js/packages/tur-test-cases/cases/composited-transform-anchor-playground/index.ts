import {
    Alignment,
    Color,
    Column,
    CompositedTransformFollower,
    CompositedTransformTarget,
    Condition,
    Container,
    CrossAxisAlignment,
    createLayerLink,
    createStore,
    derive,
    HitTestBehavior,
    MainAxisSize,
    MouseRegion,
    type Mutation,
    mutate,
    PointerInteract,
    type PointerInteractEvent,
    type PointerRegionEvent,
    Positioned,
    type ReadonlyStoreCtx,
    Row,
    SizedBox,
    Stack,
    source,
    Text,
    view,
} from "tur:std";
export const store = createStore();

// ---------------------------------------------------------------------------
// Composited-transform anchor playground.
//
// A target (dark box) is linked to a follower (red box) via `createLayerLink`.
// Two popover dropdowns pick `targetAnchor` / `followerAnchor` (all 9
// alignments); two steppers nudge `targetOffset`. Everything is reactive — the
// follower relocates the same frame the source changes.
//
// The dropdowns are built from primitives (no built-in Select element):
//   - trigger chip toggles `openMenu$`
//   - a full-canvas opaque backdrop (rendered below the panel) catches
//     outside clicks to dismiss
//   - the option list floats in the root overlay (no clipping)
// ---------------------------------------------------------------------------

const targetAnchor$ = source(Alignment.TopLeft);
const followerAnchor$ = source(Alignment.TopLeft);
const offsetX$ = source(0);
const offsetY$ = source(0);
// Single-open: opening one menu closes the other. `null` = none.
const openMenu$ = source<null | "target" | "follower">(null);
// Hovered option label within whichever menu is open (`null` = none).
const hoveredOpt$ = source<string | null>(null);

const ANCHORS: { label: string; value: Alignment }[] = [
    { label: "TopLeft", value: Alignment.TopLeft },
    { label: "TopCenter", value: Alignment.TopCenter },
    { label: "TopRight", value: Alignment.TopRight },
    { label: "CenterLeft", value: Alignment.CenterLeft },
    { label: "Center", value: Alignment.Center },
    { label: "CenterRight", value: Alignment.CenterRight },
    { label: "BottomLeft", value: Alignment.BottomLeft },
    { label: "BottomCenter", value: Alignment.BottomCenter },
    { label: "BottomRight", value: Alignment.BottomRight },
];

function labelFor(a: Alignment): string {
    return ANCHORS.find((x) => x.value === a)?.label ?? "?";
}

// `Brush`/`Color` props only accept `Color` handles (raw hex strings don't
// parse), so build a palette once and reuse.
const C = {
    target: Color.hex("#1e293b"),
    panel: Color.hex("#f1f5f9"),
    red: Color.hex("#ef4444"),
    white: Color.hex("#ffffff"),
    indigo: Color.hex("#6366f1"),
    indigoSoft: Color.hex("#e0e7ff"),
    indigoSofter: Color.hex("#eef2ff"),
    slate: Color.hex("#94a3b8"),
    slateLight: Color.hex("#cbd5e1"),
    grayLight: Color.hex("#e2e8f0"),
    shadow: Color.hex("#1e293b"),
    text: Color.hex("#0f172a"),
    textMid: Color.hex("#475569"),
    textMuted: Color.hex("#64748b"),
    targetLabel: Color.hex("#94a3b8"),
};

const click = (
    m: Mutation<[], unknown>,
): Mutation<[PointerInteractEvent], void> =>
    m as unknown as Mutation<[PointerInteractEvent], void>;

// `MouseRegion` hover callbacks carry a `PointerRegionEvent`; we ignore it.
const hover = (
    m: Mutation<[], unknown>,
): Mutation<[PointerRegionEvent], void> =>
    m as unknown as Mutation<[PointerRegionEvent], void>;

// Layout constants (deterministic — menus float at known coords below their
// triggers, so no portal/composited-transform is needed for the UI itself).
const PANEL_X = 16;
const PANEL_Y = 16;
const PANEL_W = 168;
const PAD = 8;
const TRIGGER_W = 152;
const TRIGGER_H = 28;
const MENU_W = 152;
const ROW_H = 24;
// Trigger 1 sits at the panel content origin (PANEL_X+PAD, PANEL_Y+PAD); its
// menu opens directly below it. Trigger 2 is one trigger-height + 6px below.
const MENU1_X = PANEL_X + PAD;
const MENU1_Y = PANEL_Y + PAD + TRIGGER_H;
const MENU2_X = PANEL_X + PAD;
const MENU2_Y = MENU1_Y + 6 + TRIGGER_H;

export default view(() => {
    const link = createLayerLink();
    return Stack({
        children: [
            // 1. Canvas background (also sizes the root Stack).
            SizedBox({ width: 400, height: 600 }),

            // 2. The linked target — the follower tracks this box's anchors.
            Positioned({
                left: 30,
                top: 300,
                child: CompositedTransformTarget({
                    link,
                    child: Container({
                        width: 140,
                        height: 90,
                        borderRadius: 8,
                        color: C.target,
                        alignment: Alignment.Center,
                        children: [
                            Text({
                                text: "Target",
                                fontSize: 12,
                                color: C.targetLabel,
                            }),
                        ],
                    }),
                }),
            }),

            // 3. The follower — anchors + offset are fully reactive.
            CompositedTransformFollower({
                link,
                targetAnchor: derive((ctx) => ctx.get(targetAnchor$)),
                followerAnchor: derive((ctx) => ctx.get(followerAnchor$)),
                targetOffset: derive((ctx) => ({
                    x: ctx.get(offsetX$),
                    y: ctx.get(offsetY$),
                })),
                child: Container({
                    width: 48,
                    height: 36,
                    borderRadius: 6,
                    color: C.red,
                    alignment: Alignment.Center,
                    children: [
                        Text({ text: "F", fontSize: 13, color: C.white }),
                    ],
                }),
            }),

            // 4. Click-outside backdrop (below the panel so triggers stay
            //    clickable while a menu is open; catches canvas clicks).
            Condition({
                condition: derive((ctx) => ctx.get(openMenu$) !== null),
                child: () =>
                    Positioned({
                        left: 0,
                        top: 0,
                        width: 400,
                        height: 600,
                        child: PointerInteract({
                            behavior: HitTestBehavior.Opaque,
                            onClick: click(
                                mutate((ctx) => ctx.set(openMenu$, null)),
                            ),
                            child: SizedBox({ width: 400, height: 600 }),
                        }),
                    }),
            }),

            // 5. Controls panel (triggers + steppers + readout).
            Positioned({
                left: PANEL_X,
                top: PANEL_Y,
                child: ControlsPanel(),
            }),

            // 6. Floating menus (root overlay → no clipping; on top of all).
            Positioned({
                left: MENU1_X,
                top: MENU1_Y,
                child: Condition({
                    condition: derive((ctx) => ctx.get(openMenu$) === "target"),
                    child: () => MenuList(targetAnchor$),
                }),
            }),
            Positioned({
                left: MENU2_X,
                top: MENU2_Y,
                child: Condition({
                    condition: derive(
                        (ctx) => ctx.get(openMenu$) === "follower",
                    ),
                    child: () => MenuList(followerAnchor$),
                }),
            }),
        ],
    });
});

// ---------------------------------------------------------------------------
// Widget builders
// ---------------------------------------------------------------------------

function ControlsPanel() {
    return Container({
        width: PANEL_W,
        padding: PAD,
        borderRadius: 8,
        color: C.panel,
        children: [
            Column({
                mainAxisSize: MainAxisSize.Min,
                crossAlignment: CrossAxisAlignment.Start,
                children: [
                    TriggerChip("target", targetAnchor$, "target"),
                    SizedBox({ height: 6 }),
                    TriggerChip("follower", followerAnchor$, "follower"),
                    SizedBox({ height: 12 }),
                    Stepper("offsetX", offsetX$, 5),
                    SizedBox({ height: 4 }),
                    Stepper("offsetY", offsetY$, 5),
                    SizedBox({ height: 10 }),
                    Text({
                        text: derive(
                            (ctx) =>
                                `t:${labelFor(ctx.get(targetAnchor$))}  f:${labelFor(ctx.get(followerAnchor$))}  off:(${ctx.get(offsetX$)},${ctx.get(offsetY$)})`,
                        ),
                        fontSize: 10,
                        color: C.textMuted,
                    }),
                ],
            }),
        ],
    });
}

function TriggerChip(
    label: string,
    value$: typeof targetAnchor$,
    menuKey: "target" | "follower",
) {
    return PointerInteract({
        onClick: click(
            mutate((ctx) =>
                ctx.set(
                    openMenu$,
                    ctx.get(openMenu$) === menuKey ? null : menuKey,
                ),
            ),
        ),
        child: Container({
            width: TRIGGER_W,
            height: TRIGGER_H,
            borderRadius: 6,
            borderWidth: 1,
            borderColor: C.slate,
            color: derive((ctx) =>
                ctx.get(openMenu$) === menuKey ? C.indigoSoft : C.white,
            ),
            padding: 6,
            alignment: Alignment.CenterLeft,
            children: [
                Row({
                    children: [
                        Text({
                            text: `${label}: `,
                            fontSize: 11,
                            color: C.textMid,
                        }),
                        Text({
                            text: derive((ctx) => labelFor(ctx.get(value$))),
                            fontSize: 11,
                            color: C.text,
                        }),
                        SizedBox({ width: 6 }),
                        Text({ text: "▾", fontSize: 10, color: C.slate }),
                    ],
                }),
            ],
        }),
    });
}

function MenuList(value$: typeof targetAnchor$) {
    return Container({
        width: MENU_W,
        borderRadius: 8,
        borderWidth: 1,
        borderColor: C.slateLight,
        shadowColor: C.shadow,
        shadowBlur: 14,
        shadowOffset: [0, 4],
        color: C.white,
        children: [
            Column({
                mainAxisSize: MainAxisSize.Min,
                crossAlignment: CrossAxisAlignment.Stretch,
                children: ANCHORS.map((opt) => OptionRow(opt, value$)),
            }),
        ],
    });
}

function OptionRow(
    opt: { label: string; value: Alignment },
    value$: typeof targetAnchor$,
) {
    const isSel = (ctx: ReadonlyStoreCtx) => ctx.get(value$) === opt.value;
    const isHover = (ctx: ReadonlyStoreCtx) =>
        ctx.get(hoveredOpt$) === opt.label;
    return MouseRegion({
        cursor: "pointer",
        behavior: HitTestBehavior.Translucent,
        onEnter: hover(mutate((ctx) => ctx.set(hoveredOpt$, opt.label))),
        onExit: hover(
            mutate((ctx) => {
                if (ctx.get(hoveredOpt$) === opt.label)
                    ctx.set(hoveredOpt$, null);
            }),
        ),
        child: PointerInteract({
            onClick: click(
                mutate((ctx) => {
                    ctx.set(value$, opt.value);
                    ctx.set(openMenu$, null);
                    ctx.set(hoveredOpt$, null);
                }),
            ),
            child: Container({
                width: MENU_W,
                height: ROW_H,
                padding: 6,
                color: derive((ctx) =>
                    isSel(ctx)
                        ? C.indigo
                        : isHover(ctx)
                          ? C.indigoSofter
                          : C.white,
                ),
                children: [
                    Text({
                        text: opt.label,
                        fontSize: 11,
                        color: derive((ctx) => (isSel(ctx) ? C.white : C.text)),
                    }),
                ],
            }),
        }),
    });
}

function Stepper(label: string, value$: typeof offsetX$, step: number) {
    return Row({
        children: [
            Text({ text: `${label}:`, fontSize: 11, color: C.textMid }),
            SizedBox({ width: 6 }),
            SmallButton(
                "−",
                mutate((ctx) => ctx.set(value$, ctx.get(value$) - step)),
            ),
            SizedBox({ width: 6 }),
            Text({
                text: derive((ctx) => `${ctx.get(value$)}`),
                fontSize: 11,
                color: C.text,
            }),
            SizedBox({ width: 6 }),
            SmallButton(
                "+",
                mutate((ctx) => ctx.set(value$, ctx.get(value$) + step)),
            ),
        ],
    });
}

function SmallButton(label: string, onClick: Mutation<[], void>) {
    return PointerInteract({
        onClick: click(onClick),
        child: Container({
            width: 20,
            height: 20,
            borderRadius: 4,
            color: C.grayLight,
            alignment: Alignment.Center,
            children: [Text({ text: label, fontSize: 11, color: C.text })],
        }),
    });
}
