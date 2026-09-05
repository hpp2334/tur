import {
    Color,
    Column,
    Container,
    createTextEditingController,
    derive,
    Input,
    mount,
    mutate,
    source,
    Text,
    view,
} from "tur:std";

// Demonstrates `obscureText` (password mode): each character of the value is
// rendered as `obscuringCharacter` (default "•") while the controller keeps
// the real text. Copy/Cut are suppressed. A plain Input is shown for
// comparison, and a readout echoes the controller's true value via `onInput`.
const App = view(() => {
    // Local state: the view fn runs exactly once (at build), so the atoms and
    // controllers are stable for the life of the tree — no need to hoist them
    // to module level.
    const value$ = source("hunter2");
    const pwCtrl = createTextEditingController({
        initialText: "hunter2",
        onInput: mutate((ctx, text: string) => ctx.set(value$, text)),
    });
    const plainCtrl = createTextEditingController({});

    return Container({
        padding: 32,
        children: [
            Column({
                children: [
                    Text({
                        text: "Password Input",
                        fontSize: 18,
                        color: Color.rgb(15, 23, 42),
                    }),
                    Input({
                        controller: plainCtrl,
                        placeholder: "Plain",
                        fontSize: 16,
                        width: 240,
                        height: 38,
                        queryKey: ["plain"],
                    }),
                    Input({
                        controller: pwCtrl,
                        placeholder: "Password",
                        obscureText: true,
                        fontSize: 16,
                        width: 240,
                        height: 38,
                        queryKey: ["password"],
                    }),
                    Text({
                        text: derive((ctx) => `value: "${ctx.get(value$)}"`),
                        fontSize: 12,
                        color: Color.rgb(100, 116, 139),
                    }),
                ],
            }),
        ],
    });
});

export function start() {
    mount(App);
}
