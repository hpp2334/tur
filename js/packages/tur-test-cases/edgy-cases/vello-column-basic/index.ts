import { Column, Container, component } from "@tur/edgy";

export default component(() =>
    Column({
        children: [
            Container({ width: 200, height: 50 }),
            Container({ width: 200, height: 30 }),
        ],
    }),
);
