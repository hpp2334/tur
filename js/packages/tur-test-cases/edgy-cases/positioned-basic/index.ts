import { Positioned, render, SizedBox, Stack } from "@tur/edgy";

render(() =>
    Stack({
        children: [
            Positioned({
                left: 10,
                top: 20,
                child: SizedBox({ width: 50, height: 50 }),
            }),
        ],
    }),
);
