import {
    type Brush,
    Column,
    Condition,
    Container,
    CrossAxisAlignment,
    derive,
    Each,
    type Element,
    Expanded,
    get,
    MainAxisAlignment,
    MainAxisSize,
    MouseRegion,
    mutate,
    PointerInteract,
    Row,
    ScrollView,
    SizedBox,
    set,
    Text,
} from "builtin:tur/std";
import {
    CASE_NAMES,
    edited$,
    getCaseFileNames,
    hoveredCase$,
    hoveredFile$,
    loadCase,
    selectedCase$,
    selectedFile$,
    selectFile,
    sidebarWidth$,
} from "../state";
import { tokens } from "../theme/tokens";

/** Sidebar header: small uppercase label + count of available cases. */
function SidebarHeader(): Element {
    return Container({
        padding: 14,
        children: [
            Row({
                mainAlignment: MainAxisAlignment.SpaceBetween,
                children: [
                    Text({
                        text: "CASES",
                        fontSize: 10,
                        color: tokens.text.tertiary,
                    }),
                    Text({
                        text: `${CASE_NAMES.length}`,
                        fontSize: 10,
                        color: tokens.text.tertiary,
                    }),
                ],
            }),
        ],
    });
}

/** A case row in the navigation list. Renders as a card with a left accent
 *  bar when selected; multi-file cases expand an indented file list below
 *  the row when active. */
function NavItem(name: string): Element {
    const isSelected = () => get(selectedCase$) === name;

    return Column({
        crossAlignment: CrossAxisAlignment.Stretch,
        children: [
            SizedBox({ height: 2 }),
            MouseRegion({
                cursor: "pointer",
                onEnter: mutate((_ctx, _ev) => set(hoveredCase$, name)),
                onExit: mutate((_ctx, _ev) => set(hoveredCase$, null)),
                child: PointerInteract({
                    onClick: mutate((_ctx, _ev) => loadCase(name)),
                    child: Container({
                        // Horizontal inset so the rounded card floats in
                        // the sidebar rather than bleeding to the edge.
                        padding: 8,
                        children: [
                            Row({
                                children: [
                                    // Left accent bar — only visible when
                                    // selected. Reserved 3px width either
                                    // way so the label doesn't shift. Fixed
                                    // height avoids CrossAxisAlignment.Stretch
                                    // issues in the unbounded ScrollView.
                                    Container({
                                        width: 3,
                                        height: 20,
                                        borderRadius: 2,
                                        color: derive(() =>
                                            isSelected()
                                                ? tokens.accent.solid
                                                : null,
                                        ) as unknown as Brush,
                                    }),
                                    SizedBox({ width: 8 }),
                                    Expanded({
                                        child: Container({
                                            borderRadius: 5,
                                            padding: 7,
                                            color: derive(() => {
                                                const selected = isSelected();
                                                const hovered =
                                                    get(hoveredCase$) === name;
                                                if (selected)
                                                    return hovered
                                                        ? tokens.bg.strongHover
                                                        : tokens.bg.elevated;
                                                return hovered
                                                    ? tokens.bg.hover
                                                    : null;
                                            }) as unknown as Brush,
                                            children: [
                                                Row({
                                                    mainAlignment:
                                                        MainAxisAlignment.SpaceBetween,
                                                    children: [
                                                        Text({
                                                            text: name,
                                                            fontSize: 13,
                                                            color: derive(() =>
                                                                isSelected()
                                                                    ? tokens
                                                                          .text
                                                                          .primary
                                                                    : get(
                                                                            hoveredCase$,
                                                                        ) ===
                                                                        name
                                                                      ? tokens
                                                                            .text
                                                                            .primary
                                                                      : tokens
                                                                            .text
                                                                            .body,
                                                            ),
                                                        }),
                                                        // Edited indicator —
                                                        // small coral dot when
                                                        // this case's editor
                                                        // text differs from
                                                        // its last-compiled
                                                        // version.
                                                        Condition({
                                                            condition: derive(
                                                                () =>
                                                                    get(
                                                                        edited$,
                                                                    ) &&
                                                                    isSelected(),
                                                            ),
                                                            child: () =>
                                                                Container({
                                                                    width: 6,
                                                                    height: 6,
                                                                    borderRadius: 999,
                                                                    color: tokens
                                                                        .accent
                                                                        .complement,
                                                                }),
                                                            elseChild: () =>
                                                                SizedBox({
                                                                    width: 0,
                                                                    height: 0,
                                                                }),
                                                        }),
                                                    ],
                                                }),
                                            ],
                                        }),
                                    }),
                                ],
                            }),
                        ],
                    }),
                }),
            }),
            // File sublist — only the selected multi-file case shows its
            // files, indented under the case row.
            Condition({
                condition: derive(
                    () => isSelected() && getCaseFileNames(name).length > 1,
                ),
                child: () =>
                    Container({
                        padding: 8,
                        children: [
                            // Leading indent to align files under the case
                            // label (past the accent bar + inset).
                            Row({
                                crossAlignment: CrossAxisAlignment.Start,
                                children: [
                                    SizedBox({ width: 19 }),
                                    Expanded({
                                        child: Column({
                                            crossAlignment:
                                                CrossAxisAlignment.Stretch,
                                            children: [
                                                Each({
                                                    items: derive(() =>
                                                        getCaseFileNames(
                                                            name,
                                                        ).map((filename) => ({
                                                            caseName: name,
                                                            filename,
                                                        })),
                                                    ),
                                                    build: (item) =>
                                                        FileItem(item.filename),
                                                }),
                                            ],
                                        }),
                                    }),
                                ],
                            }),
                        ],
                    }),
                elseChild: () => SizedBox({ width: 0, height: 0 }),
            }),
        ],
    });
}

/** A file tab in the nested file list. Only shown for multi-file cases. */
function FileItem(filename: string): Element {
    const isSelected = () => get(selectedFile$) === filename;
    return MouseRegion({
        cursor: "pointer",
        onEnter: mutate((_ctx, _ev) => set(hoveredFile$, filename)),
        onExit: mutate((_ctx, _ev) => set(hoveredFile$, null)),
        child: PointerInteract({
            onClick: mutate((_ctx, _ev) => selectFile(filename)),
            child: Container({
                padding: 5,
                borderRadius: 4,
                color: derive(() => {
                    const selected = isSelected();
                    const hovered = get(hoveredFile$) === filename;
                    if (selected) return tokens.bg.strongHover;
                    return hovered ? tokens.bg.hover : null;
                }) as unknown as Brush,
                children: [
                    Row({
                        mainAlignment: MainAxisAlignment.SpaceBetween,
                        children: [
                            Text({
                                text: filename,
                                fontSize: 12,
                                color: derive(() =>
                                    isSelected()
                                        ? tokens.text.primary
                                        : get(hoveredFile$) === filename
                                          ? tokens.text.primary
                                          : tokens.text.secondary,
                                ),
                            }),
                            // Active file marker — small accent dot.
                            Condition({
                                condition: derive(() => isSelected()),
                                child: () =>
                                    Container({
                                        width: 4,
                                        height: 4,
                                        borderRadius: 999,
                                        color: tokens.accent.solid,
                                    }),
                                elseChild: () =>
                                    SizedBox({ width: 0, height: 0 }),
                            }),
                        ],
                    }),
                ],
            }),
        }),
    });
}

export function Sidebar(): Element {
    return Container({
        width: derive(() => get(sidebarWidth$)),
        color: tokens.bg.panel,
        children: [
            Column({
                crossAlignment: CrossAxisAlignment.Stretch,
                children: [
                    SidebarHeader(),
                    Expanded({
                        child: ScrollView({
                            child: Column({
                                crossAlignment: CrossAxisAlignment.Stretch,
                                children: [
                                    ...CASE_NAMES.map((name) => NavItem(name)),
                                    SizedBox({ height: 8 }),
                                ],
                            }),
                        }),
                    }),
                ],
            }),
        ],
    });
}
