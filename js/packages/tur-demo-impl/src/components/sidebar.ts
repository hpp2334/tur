import {
    Column,
    Condition,
    Container,
    CrossAxisAlignment,
    derive,
    type EdgyElement,
    Expanded,
    get,
    mutate,
    PointerInteract,
    Row,
    ScrollView,
    SizedBox,
    set,
    Text,
} from "@tur/edgy";
import {
    CASE_NAMES,
    edited$,
    hoveredCase$,
    loadCase,
    selectedCase$,
} from "../state";
import { tokens } from "../theme/tokens";

function NavItem(name: string): EdgyElement {
    return PointerInteract({
        onClick: mutate(() => loadCase(name)),
        onPointerEnter: mutate(() => set(hoveredCase$, name)),
        onPointerExit: mutate(() => set(hoveredCase$, null)),
        child: Container({
            padding: 8,
            color: derive(() => {
                const selected = get(selectedCase$) === name;
                const hovered = get(hoveredCase$) === name;
                if (selected)
                    return hovered
                        ? tokens.bg.selectedHover
                        : tokens.bg.selected;
                return hovered ? tokens.bg.hover : tokens.bg.panel;
            }),
            children: [
                Row({
                    children: [
                        Text({
                            text: name,
                            fontSize: 13,
                            color: derive(() =>
                                get(selectedCase$) === name
                                    ? tokens.text.primary
                                    : get(hoveredCase$) === name
                                      ? tokens.text.primary
                                      : tokens.text.body,
                            ),
                        }),
                        // Edited indicator — small coral dot when this case's
                        // editor text differs from its last-compiled version.
                        Condition({
                            condition: derive(
                                () =>
                                    get(edited$) && get(selectedCase$) === name,
                            ),
                            child: () =>
                                Container({
                                    width: 6,
                                    height: 6,
                                    borderRadius: 999,
                                    color: tokens.accent.complement,
                                }),
                            elseChild: () => SizedBox({ width: 0, height: 0 }),
                        }),
                    ],
                }),
            ],
        }),
    });
}

export function Sidebar(): EdgyElement {
    return Container({
        width: 200,
        color: tokens.bg.panel,
        children: [
            Column({
                crossAlignment: CrossAxisAlignment.Stretch,
                children: [
                    SizedBox({ height: 8 }),
                    Expanded({
                        child: ScrollView({
                            child: Column({
                                crossAlignment: CrossAxisAlignment.Start,
                                children: CASE_NAMES.map((name) =>
                                    NavItem(name),
                                ),
                            }),
                        }),
                    }),
                ],
            }),
        ],
    });
}
