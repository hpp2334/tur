import {
    Column,
    CrossAxisAlignment,
    createStore,
    SizedBox,
    view,
} from "tur:std";

export const store = createStore();

export default view(() =>
    Column({
        crossAlignment: CrossAxisAlignment.Start,
        children: [SizedBox({ width: 100, height: 50 })],
    }),
);
