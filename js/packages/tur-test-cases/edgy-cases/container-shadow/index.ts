import { Alignment, Color, Container, Text, render } from "@tur/edgy";

render(() =>
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
