import {
    Column,
    Condition,
    Container,
    CrossAxisAlignment,
    derive,
    Each,
    type EdgyElement,
    Expanded,
    get,
    MouseRegion,
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

function NavItem(name: string): EdgyElement {
    return MouseRegion({
        cursor: "pointer",
        onEnter: mutate((_ctx, _ev) => set(hoveredCase$, name)),
        onExit: mutate((_ctx, _ev) => set(hoveredCase$, null)),
        child: PointerInteract({
            onClick: mutate((_ctx, _ev) => loadCase(name)),
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
                                        get(edited$) &&
                                        get(selectedCase$) === name,
                                ),
                                child: () =>
                                    Container({
                                        width: 6,
                                        height: 6,
                                        borderRadius: 999,
                                        color: tokens.accent.complement,
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

/** A file tab in the file list. Only shown for multi-file cases. */
function FileItem(filename: string): EdgyElement {
    return MouseRegion({
        cursor: "pointer",
        onEnter: mutate((_ctx, _ev) => set(hoveredFile$, filename)),
        onExit: mutate((_ctx, _ev) => set(hoveredFile$, null)),
        child: PointerInteract({
            onClick: mutate((_ctx, _ev) => selectFile(filename)),
            child: Container({
                padding: 8,
                color: derive(() => {
                    const selected = get(selectedFile$) === filename;
                    const hovered = get(hoveredFile$) === filename;
                    if (selected)
                        return hovered
                            ? tokens.bg.selectedHover
                            : tokens.bg.selected;
                    return hovered ? tokens.bg.hover : tokens.bg.panel;
                }),
                children: [
                    Text({
                        text: filename,
                        fontSize: 12,
                        color: derive(() =>
                            get(selectedFile$) === filename
                                ? tokens.text.primary
                                : tokens.text.secondary,
                        ),
                    }),
                ],
            }),
        }),
    });
}

export function Sidebar(): EdgyElement {
    return Container({
        width: derive(() => get(sidebarWidth$)),
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
                                children: [
                                    // Case list.
                                    ...CASE_NAMES.map((name) => NavItem(name)),
                                    // File list for the selected case (only
                                    // shown when the case has multiple files).
                                    FileListForCase(),
                                ],
                            }),
                        }),
                    }),
                ],
            }),
        ],
    });
}

/** Shows the file list for the currently selected case. Rebuilds when the
 *  selected case changes. Hidden for single-file cases. */
function FileListForCase(): EdgyElement {
    return Each({
        items: derive(() => {
            const name = get(selectedCase$);
            const files = getCaseFileNames(name);
            // Only show when there are multiple files.
            if (files.length <= 1) return [];
            return files.map((f) => ({ caseName: name, filename: f }));
        }),
        build: (item) => FileItem(item.filename),
    });
}
