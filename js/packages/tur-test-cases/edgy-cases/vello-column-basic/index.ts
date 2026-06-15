import { Column, Container, render } from "@tur/edgy";

render(() =>
    Column({
        children: [
            Container({ width: 200, height: 50 }),
            Container({ width: 200, height: 30 }),
        ],
    }),
);
