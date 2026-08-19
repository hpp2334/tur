import { Container, createStore, SizedBox, view } from "tur:std";

export const store = createStore();

export default view(() =>
    Container({
        padding: 16,
        children: [SizedBox({ width: 100, height: 100 })],
    }),
);
