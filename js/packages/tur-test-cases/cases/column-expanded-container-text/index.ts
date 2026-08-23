import { Column, Container, Expanded, mount, Text, view } from "tur:std";

// Repro #5: Container(no explicit size, padding:48) sized from a Text child,
// wrapped in Expanded, inside a Column with MainAxisSize.Max. Expected: the
// Expanded fills remaining main axis; the inner container is visible.
const App = view(() =>
    Column({
        children: [
            Expanded({
                child: Container({
                    padding: 48,
                    queryKey: ["container"],
                    children: [
                        Text({
                            text: "Empty State",
                            fontSize: 24,
                            queryKey: ["text"],
                        }),
                    ],
                }),
            }),
        ],
    }),
);

export function start() {
    mount(App);
}
