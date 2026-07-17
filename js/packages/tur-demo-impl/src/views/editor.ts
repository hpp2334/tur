import {
    Color,
    Container,
    createScrollController,
    derive,
    Each,
    type Element,
    Expanded,
    get,
    Input,
    MouseRegion,
    type Mutation,
    mutate,
    type PointerRegionEvent,
    Row,
    Scrollbar,
    ScrollView,
    set,
    source,
} from "builtin:tur/std";
import {
    editorCtrl,
    editorUndo,
    openContextMenu,
    selectedCase$,
    selectedFile$,
} from "../state";
import { tokens } from "../theme/tokens";

const editorInput: Element = Input({
    controller: editorCtrl,
    undoController: editorUndo,
    multiline: true,
    fontFamily: "monospace",
    fontSize: 13,
    color: tokens.text.code,
    cursorColor: tokens.accent.cursor,
    placeholderColor: tokens.text.placeholder,
    onContextMenu: openContextMenu,
    queryKey: ["editor-input"],
});

/** Per-instance hover state for the scrollbar — recreated every time the
 *  editor subtree rebuilds (so the source lives inside the factory). */
function scrollableEditor(): Element {
    // A fresh controller per build so it binds to the right scroll-view node
    // whenever the case (and thus the subtree) is rebuilt.
    const controller = createScrollController();
    // Light-gray track shows only while hovered.
    const trackHovered$ = source(false);
    return Row({
        children: [
            Expanded({
                child: ScrollView({ controller, child: editorInput }),
            }),
            // Dedicated 10px scrollbar column.
            MouseRegion({
                onEnter: mutate(() =>
                    set(trackHovered$, true),
                ) as unknown as Mutation<[PointerRegionEvent], void>,
                onExit: mutate(() =>
                    set(trackHovered$, false),
                ) as unknown as Mutation<[PointerRegionEvent], void>,
                child: Scrollbar({
                    controller,
                    color: tokens.text.placeholder,
                    trackColor: derive(() =>
                        get(trackHovered$)
                            ? tokens.bg.strongHover
                            : Color.rgba(0, 0, 0, 0),
                    ),
                    thickness: 10,
                    thumbRadius: 5,
                    queryKey: ["editor-scrollbar"],
                }),
            }),
        ],
    });
}

export function Editor(): Element {
    return Container({
        color: tokens.bg.code,
        padding: 12,
        children: [
            // Rebuild the editor element whenever the selected case OR file
            // changes so it re-reads the controller spans (reset by
            // loadCase / selectFile).
            Each({
                items: derive(() => [
                    { case: get(selectedCase$), file: get(selectedFile$) },
                ]),
                build: () => scrollableEditor(),
            }),
        ],
    });
}
