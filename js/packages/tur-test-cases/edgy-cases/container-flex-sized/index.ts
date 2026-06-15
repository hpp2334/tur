import {
    Alignment,
    Color,
    Column,
    Container,
    Row,
    render,
    Text,
} from "@tur/edgy";

render(() =>
    Column({
        children: [
            Row({
                children: [
                    Container({
                        width: 100,
                        height: 44,
                        color: Color.hex("#6366f1"),
                        alignment: Alignment.Center,
                        queryKey: ["btn"],
                        children: [Text({ text: "Btn", fontSize: 14 })],
                    }),
                ],
            }),
        ],
    }),
);
