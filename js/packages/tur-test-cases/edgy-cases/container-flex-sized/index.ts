import {
    Alignment,
    Color,
    Column,
    Container,
    Row,
    Text,
    view,
} from "@tur/edgy";

export default view(() =>
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
