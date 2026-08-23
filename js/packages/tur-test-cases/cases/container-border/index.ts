import {
    Alignment,
    BorderPosition,
    Color,
    Container,
    Text,
    view,
} from "tur:std";

export default view(() =>
    Container({
        width: 200,
        height: 200,
        padding: 16,
        color: Color.hex("#ffffff"),
        borderColor: Color.hex("#000000"),
        borderWidth: 2,
        borderRadius: 8,
        borderPosition: BorderPosition.Inside,
        alignment: Alignment.Center,
        children: [Text({ text: "Border", fontSize: 16 })],
    }),
);
