import { Column, Container, createStore, view } from "tur:std";

export const store = createStore();

export default view(() =>
    Column({
        children: [
            Container({ width: 200, height: 50 }),
            Container({ width: 200, height: 30 }),
        ],
    }),
);
