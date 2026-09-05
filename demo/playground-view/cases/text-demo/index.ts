import {
    Color,
    Column,
    Container,
    CrossAxisAlignment,
    derive,
    type Element,
    MainAxisSize,
    MouseRegion,
    type Mutation,
    mount,
    mutate,
    PointerInteract,
    type Readable,
    Row,
    ScrollView,
    SizedBox,
    type StoreCtx,
    source,
    Text,
    type Val,
    view,
} from "tur:std";

// Comprehensive `Text` demo: font size, font weight, color, rich-text spans
// (`weight` / `italic` / `underline` / `fontSize` / `color` per run), and the
// interactive `maxLines` + `overflow` ("clip" / "ellipsis" / "visible") toggle.
//
// Note: `MainAxisSize` is a numeric enum (mirrors a TS enum), so it must be
// passed as `MainAxisSize.Min` — the string `"min"` would silently fail to
// decode and fall back to `Max`, making every card expand to the viewport
// height instead of shrinking to its content.

const C = {
    pageBg: Color.hex("#f8fafc"),
    cardBg: Color.hex("#ffffff"),
    cardBorder: Color.hex("#e2e8f0"),
    text: Color.hex("#0f172a"),
    textMuted: Color.hex("#64748b"),
    accent: Color.hex("#4f46e5"),
    accentFg: Color.hex("#ffffff"),
    rose: Color.hex("#e11d48"),
    emerald: Color.hex("#059669"),
    amber: Color.hex("#d97706"),
};

const BROWN = "The quick brown fox jumps over the lazy dog.";

/// A titled, content-sized card. `mainAxisSize: MainAxisSize.Min` keeps the
/// card as short as its children (the bug fixed: a string `"min"` here used
/// to decode to `None` → `Max` → full viewport height).
function Section({
    title,
    children,
}: {
    title: string;
    children: Element[];
}): Element {
    return Container({
        padding: 14,
        borderRadius: 10,
        color: C.cardBg,
        borderColor: C.cardBorder,
        borderWidth: 1,
        children: [
            Column({
                crossAlignment: CrossAxisAlignment.Start,
                mainAxisSize: MainAxisSize.Min,
                children: [
                    Text({ text: title, fontSize: 11, color: C.textMuted }),
                    SizedBox({ height: 10 }),
                    ...children,
                ],
            }),
        ],
    });
}

function PrimaryButton({
    label,
    onClick,
}: {
    label: Val<string>;
    onClick: Mutation<[], void>;
}): Element {
    return MouseRegion({
        cursor: "pointer",
        child: PointerInteract({
            onClick: mutate((ctx, _ev) => ctx.set(onClick)),
            child: Container({
                padding: 10,
                borderRadius: 8,
                color: C.accent,
                children: [
                    Text({
                        text: label,
                        fontSize: 13,
                        color: C.accentFg,
                    }),
                ],
            }),
        }),
    });
}

function OverflowCard({
    overflow,
    maxLines$,
}: {
    overflow: "clip" | "ellipsis" | "visible";
    maxLines$: Readable<number>;
}): Element {
    return Container({
        width: 100,
        padding: 8,
        borderRadius: 8,
        color: C.cardBg,
        borderColor: C.cardBorder,
        borderWidth: 1,
        children: [
            Column({
                crossAlignment: CrossAxisAlignment.Start,
                mainAxisSize: MainAxisSize.Min,
                children: [
                    Text({ text: overflow, fontSize: 11, color: C.textMuted }),
                    SizedBox({ height: 6 }),
                    Text({
                        text: BROWN,
                        fontSize: 12,
                        color: C.text,
                        maxLines: derive((ctx) => ctx.get(maxLines$)),
                        overflow,
                    }),
                ],
            }),
        ],
    });
}

const App = view(() => {
    // Local state: the view fn runs exactly once (at build), so the atom and
    // mutation are stable for the life of the tree — no need to hoist them
    // to module level.
    const maxLines$ = source<number>(2);
    const cycleMaxLines = mutate((ctx: StoreCtx) => {
        const cur = ctx.get(maxLines$);
        // 2 → 1 → 3 → 2 (cycles through interesting truncation regimes).
        ctx.set(maxLines$, cur === 2 ? 1 : cur === 1 ? 3 : 2);
    });

    return Container({
        color: C.pageBg,
        children: [
            ScrollView({
                child: Column({
                    crossAlignment: CrossAxisAlignment.Stretch,
                    mainAxisSize: MainAxisSize.Min,
                    children: [
                        SizedBox({ height: 16 }),
                        Text({ text: "Text", fontSize: 22, color: C.text }),
                        SizedBox({ height: 4 }),
                        Text({
                            text: "size · weight · color · spans · overflow",
                            fontSize: 12,
                            color: C.textMuted,
                        }),
                        SizedBox({ height: 16 }),
                        Section({
                            title: "FONT SIZE",
                            children: [
                                Row({
                                    crossAlignment: CrossAxisAlignment.End,
                                    mainAxisSize: MainAxisSize.Min,
                                    children: [
                                        Text({
                                            text: "Aa",
                                            fontSize: 10,
                                            color: C.text,
                                        }),
                                        SizedBox({ width: 12 }),
                                        Text({
                                            text: "Aa",
                                            fontSize: 14,
                                            color: C.text,
                                        }),
                                        SizedBox({ width: 12 }),
                                        Text({
                                            text: "Aa",
                                            fontSize: 20,
                                            color: C.text,
                                        }),
                                        SizedBox({ width: 12 }),
                                        Text({
                                            text: "Aa",
                                            fontSize: 28,
                                            color: C.text,
                                        }),
                                    ],
                                }),
                            ],
                        }),
                        SizedBox({ height: 12 }),
                        Section({
                            title: "FONT WEIGHT",
                            children: [
                                Text({
                                    text: BROWN,
                                    fontSize: 14,
                                    fontWeight: 300,
                                    color: C.text,
                                }),
                                SizedBox({ height: 4 }),
                                Text({
                                    text: BROWN,
                                    fontSize: 14,
                                    fontWeight: 400,
                                    color: C.text,
                                }),
                                SizedBox({ height: 4 }),
                                Text({
                                    text: BROWN,
                                    fontSize: 14,
                                    fontWeight: 700,
                                    color: C.text,
                                }),
                                SizedBox({ height: 4 }),
                                Text({
                                    text: BROWN,
                                    fontSize: 14,
                                    fontWeight: 900,
                                    color: C.text,
                                }),
                            ],
                        }),
                        SizedBox({ height: 12 }),
                        Section({
                            title: "COLOR",
                            children: [
                                Row({
                                    mainAxisSize: MainAxisSize.Min,
                                    children: [
                                        Text({
                                            text: "slate",
                                            fontSize: 14,
                                            color: C.text,
                                        }),
                                        SizedBox({ width: 12 }),
                                        Text({
                                            text: "rose",
                                            fontSize: 14,
                                            color: C.rose,
                                        }),
                                        SizedBox({ width: 12 }),
                                        Text({
                                            text: "emerald",
                                            fontSize: 14,
                                            color: C.emerald,
                                        }),
                                        SizedBox({ width: 12 }),
                                        Text({
                                            text: "amber",
                                            fontSize: 14,
                                            color: C.amber,
                                        }),
                                    ],
                                }),
                            ],
                        }),
                        SizedBox({ height: 12 }),
                        Section({
                            title: "RICH TEXT (SPANS)",
                            children: [
                                Text({
                                    fontSize: 14,
                                    spans: [
                                        { content: "The ", color: C.text },
                                        {
                                            content: "quick ",
                                            color: C.text,
                                            italic: true,
                                        },
                                        {
                                            content: "brown ",
                                            color: C.amber,
                                            weight: 700,
                                        },
                                        {
                                            content: "fox ",
                                            color: C.text,
                                            underline: true,
                                        },
                                        {
                                            content: "jumps ",
                                            color: C.text,
                                            fontSize: 18,
                                        },
                                        {
                                            content: "over the ",
                                            color: C.textMuted,
                                        },
                                        {
                                            content: "lazy",
                                            color: C.emerald,
                                            weight: 700,
                                        },
                                        {
                                            content: " dog.",
                                            color: C.textMuted,
                                        },
                                    ],
                                }),
                            ],
                        }),
                        SizedBox({ height: 12 }),
                        Section({
                            title: "OVERFLOW",
                            children: [
                                Text({
                                    text: derive(
                                        (ctx) =>
                                            `maxLines = ${ctx.get(maxLines$)}  ·  width = 100px`,
                                    ),
                                    fontSize: 11,
                                    color: C.textMuted,
                                }),
                                SizedBox({ height: 8 }),
                                Row({
                                    crossAlignment: CrossAxisAlignment.Start,
                                    mainAxisSize: MainAxisSize.Min,
                                    children: [
                                        OverflowCard({
                                            overflow: "clip",
                                            maxLines$,
                                        }),
                                        SizedBox({ width: 8 }),
                                        OverflowCard({
                                            overflow: "ellipsis",
                                            maxLines$,
                                        }),
                                        SizedBox({ width: 8 }),
                                        OverflowCard({
                                            overflow: "visible",
                                            maxLines$,
                                        }),
                                    ],
                                }),
                                SizedBox({ height: 10 }),
                                PrimaryButton({
                                    label: derive(
                                        (ctx) =>
                                            `cycle maxLines (now ${ctx.get(maxLines$)})`,
                                    ),
                                    onClick: cycleMaxLines,
                                }),
                            ],
                        }),
                        SizedBox({ height: 16 }),
                    ],
                }),
            }),
        ],
    });
});

export function start() {
    mount(App);
}
