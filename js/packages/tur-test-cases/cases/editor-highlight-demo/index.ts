import {
    Color,
    Column,
    Container,
    CrossAxisAlignment,
    createTextEditingController,
    Expanded,
    Input,
    MainAxisAlignment,
    Text,
    view,
} from "tur:std";

// A controller pre-populated with a code-like snippet, split into spans whose
// colors simulate syntax highlighting (keyword / identifier / number / punct).
const ctrl = createTextEditingController();
ctrl.setSpans([
    { content: "const", color: Color.hex("#c678dd") }, // keyword (purple)
    { content: " count", color: Color.hex("#61afef") }, // identifier (blue)
    { content: " = ", color: Color.hex("#abb2bf") }, // punctuation (grey)
    { content: "0", color: Color.hex("#d19a66") }, // number (orange)
    { content: ";\n", color: Color.hex("#abb2bf") },
    { content: "function", color: Color.hex("#c678dd") },
    { content: " inc", color: Color.hex("#61afef") },
    { content: "(", color: Color.hex("#abb2bf") },
    { content: ")", color: Color.hex("#abb2bf") },
    { content: " { count", color: Color.hex("#e5c07b") },
    { content: "++", color: Color.hex("#56b6c2") },
    { content: "; }", color: Color.hex("#abb2bf") },
]);

export default view(() =>
    Expanded({
        child: Container({
            color: Color.hex("#282c34"),
            padding: 20,
            children: [
                Column({
                    crossAlignment: CrossAxisAlignment.Start,
                    children: [
                        Text({
                            text: "Editor — colored spans + monospace",
                            fontSize: 14,
                            color: Color.hex("#848da5"),
                        }),
                        Container({ height: 12 }),
                        Input({
                            controller: ctrl,
                            multiline: true,
                            fontFamily: "monospace",
                            fontSize: 18,
                            color: Color.hex("#abb2bf"),
                            width: 360,
                            height: 120,
                        }),
                    ],
                }),
            ],
        }),
    }),
);
