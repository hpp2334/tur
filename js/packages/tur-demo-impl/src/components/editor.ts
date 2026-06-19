import {
    type EdgyElement,
    Container,
    Expanded,
    InputEdgy,
    Row,
    ScrollView,
    Scrollbar,
    Switch,
    createScrollController,
} from "@tur/edgy";
import { tokens } from "../theme/tokens";
import { CASE_NAMES, editorCtrl, selectedCase$ } from "../state";

const editorInput: EdgyElement = InputEdgy({
    controller: editorCtrl,
    multiline: true,
    fontFamily: "monospace",
    fontSize: 13,
    color: tokens.text.code,
    cursorColor: tokens.accent.cursor,
    placeholderColor: tokens.text.placeholder,
    queryKey: ["editor-input"],
});

/** A scrollable editor pane: a `ScrollView` next to a draggable `Scrollbar`
 *  that shares the same controller. (A Row column rather than an overlay,
 *  because `Positioned` doesn't honor `right`/`bottom` for placement.) */
function scrollableEditor(): EdgyElement {
    // A fresh controller per build so it binds to the right scroll-view node
    // whenever the case (and thus the subtree) is rebuilt.
    const controller = createScrollController();
    return Row({
        children: [
            Expanded({
                child: ScrollView({ controller, child: editorInput }),
            }),
            // Dedicated 10px scrollbar column.
            Scrollbar({
                controller,
                color: tokens.text.placeholder,
                thickness: 10,
                thumbRadius: 5,
                queryKey: ["editor-scrollbar"],
            }),
        ],
    });
}

export function Editor(): EdgyElement {
    return Container({
        color: tokens.bg.code,
        padding: 12,
        children: [
            // Rebuild the editor element whenever the selected case changes
            // so it re-reads the controller spans (reset by loadCase).
            Switch({
                value: selectedCase$,
                cases: CASE_NAMES.map((name) => ({
                    key: name,
                    child: () => scrollableEditor(),
                })),
                fallback: () => scrollableEditor(),
            }),
        ],
    });
}
