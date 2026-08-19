import {
    Alignment,
    type Color,
    Column,
    Container,
    CrossAxisAlignment,
    type Element,
    Image,
    Input,
    MainAxisSize,
    MouseRegion,
    type Mutation,
    mutate,
    PointerInteract,
    SizedBox,
    Text,
    type TextController,
} from "tur:std";
import { COLORS } from "./theme";

/** Standard text / accent button. `onClick` is a mutation declaration the
 *  button dispatches on click (with the mutation ctx). */
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
    onClick: Mutation<[], void>;
    padding?: number;
    shadow?: boolean;
}): Element {
    return MouseRegion({
        cursor: "pointer",
        child: PointerInteract({
            onClick: mutate((ctx, _ev) => ctx.set(onClick)),
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
    onClick: Mutation<[], void>;
}): Element {
    return MouseRegion({
        cursor: "pointer",
        child: PointerInteract({
            onClick: mutate((ctx, _ev) => ctx.set(onClick)),
            child: Container({
                width: 32,
                height: 32,
                borderRadius: 7,
                alignment: Alignment.Center,
                color: COLORS.subtleButton,
                children: [
                    Image({
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
                    Input({
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
