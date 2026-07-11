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
    MainAxisSize,
    MouseRegion,
    mutate,
    PointerInteract,
    Row,
    set,
    Text,
} from "builtin:tur/std";
import { type MobileTab, mobileTab$ } from "../state";
import { tokens } from "../theme/tokens";

/** A single bottom-tab button. Fills an equal third of the bar (`Expanded`);
 *  the active tab shows a 2px accent indicator at the top and an
 *  accent-colored label. Tapping sets `mobileTab$`. */
function MobileTabButton(tab: MobileTab, label: string): Element {
    const isActive = () => get(mobileTab$) === tab;
    return Expanded({
        child: MouseRegion({
            cursor: "pointer",
            child: PointerInteract({
                onClick: mutate((_ctx, _ev) => set(mobileTab$, tab)),
                child: Container({
                    children: [
                        Column({
                            crossAlignment: CrossAxisAlignment.Stretch,
                            children: [
                                // Top accent indicator — full width, 2px.
                                Container({
                                    height: 2,
                                    color: derive(() =>
                                        isActive()
                                            ? tokens.accent.solid
                                            : Color.rgba(0, 0, 0, 0),
                                    ),
                                }),
                                // Label, centered.
                                Container({
                                    padding: 12,
                                    children: [
                                        Row({
                                            mainAlignment:
                                                MainAxisAlignment.Center,
                                            mainAxisSize: MainAxisSize.Min,
                                            children: [
                                                Text({
                                                    text: label,
                                                    fontSize: 12,
                                                    color: derive(() =>
                                                        isActive()
                                                            ? tokens.accent
                                                                  .solid
                                                            : tokens.text
                                                                  .secondary,
                                                    ),
                                                }),
                                            ],
                                        }),
                                    ],
                                }),
                            ],
                        }),
                    ],
                }),
            }),
        }),
    });
}

/** Bottom tab bar for mobile: switches the single visible pane between
 *  Cases / Edit / View. Replaces the desktop toolbar's Split/Edit/View
 *  segmented control on narrow screens. */
export function MobileTabBar(): Element {
    return Container({
        // Fixed height: the bar's Row contains only `Expanded` children, and a
        // flex with only flex-children takes the full available cross-axis
        // (height) — without this it would consume the whole pane and starve
        // the editor above. 44px is a comfortable touch target.
        height: 44,
        color: tokens.bg.elevated,
        borderColor: tokens.border.subtle,
        borderWidth: 1,
        children: [
            Row({
                children: [
                    MobileTabButton("cases", "Cases"),
                    MobileTabButton("edit", "Edit"),
                    MobileTabButton("view", "View"),
                ],
            }),
        ],
    });
}
