import { createStore, SizedBox, Stack, view } from "tur:std";

export const store = createStore();

export default view(() =>
    Stack({
        children: [
            SizedBox({ width: 100, height: 100 }),
            SizedBox({ width: 200, height: 200 }),
        ],
    }),
);
