import {
    Alignment,
    type Color,
    Column,
    Container,
    CrossAxisAlignment,
    type Element,
    ImageEdgy,
    InputEdgy,
    MainAxisSize,
    MouseRegion,
    mutate,
    PointerInteract,
    SizedBox,
    type StoreCtx,
    type TextController,
    Text,
} from "builtin:tur/core";
import { COLORS } from "./theme";

/** Standard text / accent button. `onClick` runs within a mutation context. */
export function Button({
    label,
    bg,
    fg,
    onClick,
    padding = 9,
    shadow = false,
}: {
    label: string;
    bg: Color;
    fg: Color;
    onClick: (ctx: StoreCtx) => void;
    padding?: number;
    shadow?: boolean;
}): Element {
    return MouseRegion({
        cursor: "pointer",
        child: PointerInteract({
            onClick: mutate((ctx: StoreCtx, _ev) => onClick(ctx)),
            child: Container({
                padding,
                borderRadius: 7,
                color: bg,
                shadowColor: shadow ? COLORS.shadowSm : undefined,
                shadowBlur: shadow ? 4 : undefined,
                shadowOffset: shadow ? [0, 1] : undefined,
                children: [Text({ text: label, fontSize: 13, color: fg })],
            }),
        }),
    });
}

/** Square icon button (toolbar / nav). */
export function IconButton({
    resourceId,
    onClick,
}: {
    resourceId: number;
    onClick: (ctx: StoreCtx) => void;
}): Element {
    return MouseRegion({
        cursor: "pointer",
        child: PointerInteract({
            onClick: mutate((ctx: StoreCtx, _ev) => onClick(ctx)),
            child: Container({
                width: 32,
                height: 32,
                borderRadius: 7,
                alignment: Alignment.Center,
                color: COLORS.subtleButton,
                children: [
                    ImageEdgy({
                        resourceId,
                        width: 16,
                        height: 16,
                        queryKey: ["ico"],
                    }),
                ],
            }),
        }),
    });
}

/** Labelled single-line text field bound to a `TextEditingController`. */
export function Field({
    label,
    controller,
    placeholder,
}: {
    label: string;
    controller: TextController;
    placeholder: string;
}): Element {
    return Column({
        crossAlignment: CrossAxisAlignment.Stretch,
        mainAxisSize: MainAxisSize.Min,
        children: [
            Text({ text: label, fontSize: 11, color: COLORS.textSubtle }),
            SizedBox({ height: 6 }),
            Container({
                borderRadius: 7,
                padding: 9,
                color: COLORS.inputBg,
                borderColor: COLORS.inputBorder,
                borderWidth: 1,
                children: [
                    InputEdgy({
                        controller,
                        placeholder,
                        fontSize: 14,
                        color: COLORS.text,
                        placeholderColor: COLORS.textSubtle,
                        cursorColor: COLORS.accent,
                        queryKey: ["field"],
                    }),
                ],
            }),
        ],
    });
}
