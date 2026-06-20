import {
    Container,
    createScrollController,
    derive,
    Each,
    type EdgyElement,
    Expanded,
    get,
    InputEdgy,
    Row,
    Scrollbar,
    ScrollView,
} from "@tur/edgy";
import { editorCtrl, selectedCase$, selectedFile$ } from "../state";
import { tokens } from "../theme/tokens";

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
