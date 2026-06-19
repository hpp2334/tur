import { Container, type EdgyElement, InputEdgy, Switch } from "@tur/edgy";
import { CASE_NAMES, editorCtrl, selectedCase$ } from "../state";
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
                    child: () => editorInput,
                })),
                fallback: () => editorInput,
            }),
        ],
    });
}
