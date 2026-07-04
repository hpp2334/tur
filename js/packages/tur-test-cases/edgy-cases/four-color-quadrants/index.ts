import { Color, Container, Positioned, Stack, view } from "builtin:tur/core";

export default view(() =>
    Stack({
        children: [
            Positioned({
                left: 0,
                top: 0,
                child: Container({
                    width: 100,
                    height: 100,
                    color: Color.hex("#ff0000"),
                }),
            }),
            Positioned({
                left: 100,
                top: 0,
                child: Container({
                    width: 100,
                    height: 100,
                    color: Color.hex("#00ff00"),
                }),
            }),
            Positioned({
                left: 0,
                top: 100,
                child: Container({
                    width: 100,
                    height: 100,
                    color: Color.hex("#0000ff"),
                }),
            }),
            Positioned({
                left: 100,
                top: 100,
                child: Container({
                    width: 100,
                    height: 100,
                    color: Color.hex("#ffff00"),
                }),
            }),
        ],
    }),
);
