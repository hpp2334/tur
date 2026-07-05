import { Column, Container, view } from "builtin:tur/std";

export default view(() =>
    Column({
        children: [
            Container({ width: 200, height: 50 }),
            Container({ width: 200, height: 30 }),
        ],
    }),
);
