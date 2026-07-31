import { ClipBehavior, Color, Container, view } from "tur:std";

export default view(() =>
    Container({
        width: 200,
        height: 200,
        borderRadius: 40,
        clipBehavior: ClipBehavior.HardEdge,
        children: [
            Container({
                width: 200,
                height: 200,
                color: Color.hex("#ff0000"),
            }),
        ],
    }),
);
