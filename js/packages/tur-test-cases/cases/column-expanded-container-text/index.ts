import { Column, Container, createStore, Expanded, Text, view } from "tur:std";

export const store = createStore();

// Repro #5: Container(no explicit size, padding:48) sized from a Text child,
// wrapped in Expanded, inside a Column with MainAxisSize.Max. Expected: the
// Expanded fills remaining main axis; the inner container is visible.
export default view(() =>
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
