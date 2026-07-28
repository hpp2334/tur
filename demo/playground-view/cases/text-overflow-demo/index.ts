import {
    Color,
    Column,
    Container,
    CrossAxisAlignment,
    derive,
    type Element,
    Expanded,
    get,
    MainAxisAlignment,
    MouseRegion,
    mutate,
    PointerInteract,
    Row,
    SizedBox,
    type StoreCtx,
    source,
    Text,
    view,
} from "tur:std";

// Demo of `Text` `maxLines` + `overflow` ("clip" / "ellipsis" / "visible").
// Three boxes side-by-side share the same long text + a 140px width; only
// their `overflow` mode differs. A toggle flips `maxLines` between 2 and 99
// so the viewer can see truncation engage and disengage.

const COLORS = {
    pageBg: Color.hex("#f8fafc"),
    cardBg: Color.hex("#ffffff"),
    cardBorder: Color.hex("#e2e8f0"),
    text: Color.hex("#0f172a"),
    textMuted: Color.hex("#64748b"),
    accent: Color.hex("#4f46e5"),
    accentFg: Color.hex("#ffffff"),
    subtle: Color.hex("#f1f5f9"),
};

const LONG_TEXT =
    "The quick brown fox jumps over the lazy dog repeatedly while a small daemon quietly recompiles the workspace.";

const maxLines$ = source<number>(2);

function cycleMaxLines(ctx: StoreCtx): void {
    const cur = ctx.get(maxLines$);
    // 2 → 1 → 3 → 2 (cycles through interesting truncation regimes).
    const next = cur === 2 ? 1 : cur === 1 ? 3 : 2;
    ctx.set(maxLines$, next);
}

function PrimaryButton({
    label,
    onClick,
}: {
    label: string;
    onClick: (ctx: StoreCtx) => void;
}): Element {
    return MouseRegion({
        cursor: "pointer",
        child: PointerInteract({
            onClick: mutate((ctx, _ev) => onClick(ctx)),
            child: Container({
                padding: 10,
                borderRadius: 8,
                color: COLORS.accent,
                children: [
                    Text({
                        text: label,
                        fontSize: 13,
                        color: COLORS.accentFg,
                    }),
                ],
            }),
        }),
    });
}

function TextCard({
    title,
    overflow,
}: {
    title: string;
    overflow: "clip" | "ellipsis" | "visible";
}): Element {
    return Container({
        width: 140,
        padding: 10,
        borderRadius: 8,
        color: COLORS.cardBg,
        borderColor: COLORS.cardBorder,
        borderWidth: 1,
        children: [
            Column({
                crossAlignment: CrossAxisAlignment.Start,
                mainAxisSize: "min",
                children: [
                    Text({
                        text: title,
                        fontSize: 11,
                        color: COLORS.textMuted,
                    }),
                    SizedBox({ height: 6 }),
                    Text({
                        text: LONG_TEXT,
                        fontSize: 14,
                        color: COLORS.text,
                        maxLines: derive(() => get(maxLines$)),
                        overflow,
                    }),
                ],
            }),
        ],
    });
}

export default view(() =>
    Container({
        color: COLORS.pageBg,
        alignment: "center",
        children: [
            Column({
                mainAlignment: MainAxisAlignment.Center,
                crossAlignment: CrossAxisAlignment.Center,
                mainAxisSize: "min",
                children: [
                    Text({
                        text: "Text overflow demo",
                        fontSize: 18,
                        color: COLORS.text,
                    }),
                    SizedBox({ height: 4 }),
                    Text({
                        text: derive(
                            () => `maxLines = ${get(maxLines$)}  ·  width = 140px`,
                        ),
                        fontSize: 12,
                        color: COLORS.textMuted,
                    }),
                    SizedBox({ height: 14 }),
                    Row({
                        crossAlignment: CrossAxisAlignment.Start,
                        mainAxisSize: "min",
                        children: [
                            TextCard({ title: "clip", overflow: "clip" }),
                            SizedBox({ width: 10 }),
                            TextCard({
                                title: "ellipsis",
                                overflow: "ellipsis",
                            }),
                            SizedBox({ width: 10 }),
                            TextCard({
                                title: "visible",
                                overflow: "visible",
                            }),
                        ],
                    }),
                    SizedBox({ height: 18 }),
                    PrimaryButton({
                        label: derive(
                            () => `cycle maxLines (now ${get(maxLines$)})`,
                        ),
                        onClick: cycleMaxLines,
                    }),
                ],
            }),
        ],
    }),
);
