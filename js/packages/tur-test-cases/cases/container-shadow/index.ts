import { Alignment, Color, Container, createStore, Text, view } from "tur:std";

export const store = createStore();

export default view(() =>
    Container({
        width: 200,
        height: 200,
        color: Color.hex("#ffffff"),
        borderRadius: 8,
        shadowColor: Color.rgba(0, 0, 0, 80),
        shadowOffset: [4, 4],
        shadowBlur: 12,
        alignment: Alignment.Center,
        children: [Text({ text: "Shadow", fontSize: 16 })],
    }),
);
