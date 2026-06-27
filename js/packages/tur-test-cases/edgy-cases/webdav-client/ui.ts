import {
    Alignment,
    type Color,
    Column,
    Container,
    CrossAxisAlignment,
    type EdgyElement,
    HitTestBehavior,
    ImageEdgy,
    InputEdgy,
    MainAxisSize,
    MouseRegion,
    mutate,
    PointerInteract,
    SizedBox,
    type StoreCtx,
    Text,
} from "@tur/edgy";
import { COLORS } from "./theme";

/** Standard text / accent button. `onClick` runs within a mutation context. */
export function Button({
    label,
    bg,
    fg,
    onClick,
    padding = 9,
}: {
    label: string;
    bg: Color;
    fg: Color;
    onClick: (ctx: StoreCtx) => void;
    padding?: number;
}): EdgyElement {
    return MouseRegion({
        cursor: "pointer",
        child: PointerInteract({
            onClick: mutate((ctx: StoreCtx, _ev) => onClick(ctx)),
            child: Container({
                padding,
                borderRadius: 7,
                color: bg,
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
}): EdgyElement {
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
    controller: unknown;
    placeholder: string;
}): EdgyElement {
    return Column({
        crossAlignment: CrossAxisAlignment.Stretch,
        mainAxisSize: MainAxisSize.Min,
        children: [
            Text({ text: label, fontSize: 11, color: COLORS.textMuted }),
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
                        placeholderColor: COLORS.textMuted,
                        cursorColor: COLORS.accent,
                        queryKey: ["field"],
                    }),
                ],
            }),
        ],
    });
}

/** Full-screen scrim + centered card that swallows in-card clicks and
 *  dismisses on backdrop click. Mirrors todolist's `ModalShell`. */
export function ModalShell({
    onBackdropClick,
    card,
}: {
    onBackdropClick: (ctx: StoreCtx) => void;
    card: EdgyElement;
}): EdgyElement {
    return PointerInteract({
        behavior: HitTestBehavior.Opaque,
        onClick: mutate((ctx: StoreCtx, _ev) => onBackdropClick(ctx)),
        child: Container({
            color: COLORS.backdrop,
            alignment: Alignment.Center,
            children: [
                PointerInteract({
                    behavior: HitTestBehavior.Opaque,
                    onClick: mutate((_ctx: StoreCtx, _ev) => {
                        /* swallow click inside card */
                    }),
                    child: card,
                }),
            ],
        }),
    });
}
