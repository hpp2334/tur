import { createStore, Positioned, SizedBox, Stack, view } from "tur:std";

export const store = createStore();

export default view(() =>
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
