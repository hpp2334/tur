import {
    Column,
    Condition,
    Container,
    derive,
    type Element,
    Expanded,
    Image,
    MainAxisSize,
    MouseRegion,
    type Mutation,
    mutate,
    PointerInteract,
    type PointerInteractEvent,
    Positioned,
    Row,
    SizedBox,
    Stack,
    Text,
} from "tur:std";
import {
    closeContextMenu,
    contextMenuOpen$,
    contextMenuX$,
    contextMenuY$,
    copySelection,
    cutSelection,
    pasteFromClipboard,
    selectAll,
} from "../state";
import { tokens } from "../theme/tokens";
import { copyIconId, cutIconId, pasteIconId } from "./icons";

// Context menu — overlaid via a Stack at the canvas root. Anchored with
// `left`/`top` only (Positioned doesn't honor `right`/`bottom`). A full-
// viewport transparent scrim behind the menu captures the click-outside.

interface MenuItemSpec {
    label: string;
    iconId?: number;
    shortcut?: string;
    /** Zero-arg mutations (`mutate(() => …)`) are adapted to the
     *  `Mutation<[PointerInteractEvent]>` signature that `PointerInteract.onClick`
     *  requires — the click event is simply ignored at runtime. */
    onClick: Mutation<[], void>;
    danger?: boolean;
}

/** Like a `cond` helper but for elements — render `child` only when `value`
 *  is defined. Used to optionally include an icon column. */
function ifDefined<T>(value: T | undefined, build: (v: T) => Element): Element {
    if (value === undefined) {
        return SizedBox({ width: 0, height: 0 });
    }
    return build(value);
}

function menuItem(spec: MenuItemSpec): Element {
    // Cast zero-arg mutation → 1-arg. The runtime ignores the event arg.
    const click = spec.onClick as unknown as Mutation<
        [PointerInteractEvent],
        void
    >;
    return MouseRegion({
        cursor: "pointer",
        child: PointerInteract({
            onClick: click,
            child: Container({
                height: 32,
                padding: 8,
                children: [
                    Row({
                        children: [
                            ifDefined(spec.iconId, (id) =>
                                Container({
                                    width: 16,
                                    children: [
                                        Image({
                                            resourceId: id,
                                            width: 14,
                                            height: 14,
                                        }),
                                    ],
                                }),
                            ),
                            SizedBox({
                                width: spec.iconId !== undefined ? 8 : 24,
                            }),
                            Text({
                                text: spec.label,
                                fontSize: 12,
                                color: spec.danger
                                    ? tokens.text.onDanger
                                    : tokens.text.primary,
                            }),
                            Expanded({ child: SizedBox({ height: 1 }) }),
                            ifDefined(spec.shortcut, (sc) =>
                                Text({
                                    text: sc,
                                    fontSize: 11,
                                    color: tokens.text.tertiary,
                                }),
                            ),
                        ],
                    }),
                ],
            }),
        }),
    });
}

function menuItems(): Element {
    return Container({
        width: 200,
        padding: 4,
        borderRadius: 8,
        color: tokens.bg.elevated,
        borderColor: tokens.border.subtle,
        borderWidth: 1,
        shadowColor: tokens.shadow.sm,
        shadowBlur: 12,
        shadowOffset: [0, 4],
        children: [
            Column({
                mainAxisSize: MainAxisSize.Min,
                children: [
                    menuItem({
                        label: "Cut",
                        iconId: cutIconId,
                        shortcut: "⌘X",
                        onClick: cutSelection,
                    }),
                    menuItem({
                        label: "Copy",
                        iconId: copyIconId,
                        shortcut: "⌘C",
                        onClick: copySelection,
                    }),
                    menuItem({
                        label: "Paste",
                        iconId: pasteIconId,
                        shortcut: "⌘V",
                        onClick: pasteFromClipboard,
                    }),
                    Container({
                        height: 1,
                        color: tokens.border.subtle,
                    }),
                    menuItem({
                        label: "Select All",
                        shortcut: "⌘A",
                        onClick: selectAll,
                    }),
                ],
            }),
        ],
    });
}

/** The overlay root — should be the last child of a Stack at the canvas
 *  root so it paints on top. Renders nothing when the menu is closed. */
export function ContextMenuOverlay(): Element {
    return Condition({
        condition: derive((ctx) => ctx.get(contextMenuOpen$)),
        child: () =>
            Stack({
                children: [
                    // Full-viewport click-outside scrim. A large transparent
                    // PointerInteract captures clicks and closes the menu.
                    Positioned({
                        left: 0,
                        top: 0,
                        width: 100000,
                        height: 100000,
                        child: PointerInteract({
                            onClick: closeContextMenu as unknown as Mutation<
                                [PointerInteractEvent],
                                void
                            >,
                            child: Container({
                                width: 1,
                                height: 1,
                            }),
                        }),
                    }),
                    Positioned({
                        left: derive((ctx) => ctx.get(contextMenuX$)),
                        top: derive((ctx) => ctx.get(contextMenuY$)),
                        child: menuItems(),
                    }),
                ],
            }),
    });
}
