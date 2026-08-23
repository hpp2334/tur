import { Column, Container, mount, Text, view } from "tur:std";

// Repro #3/#4: Container(no explicit size, padding:48) as child of Column,
// sizing from a Text child's intrinsic measurement. Expected: container has
// a non-zero width and height (text height + padding).
const App = view(() =>
    Column({
        children: [
            Container({
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
        ],
    }),
);

export function start() {
    mount(App);
}
