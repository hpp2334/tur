import {
    Column,
    CrossAxisAlignment,
    createStore,
    MainAxisAlignment,
    SizedBox,
    view,
} from "tur:std";

export const store = createStore();

export default view(() =>
    Column({
        mainAlignment: MainAxisAlignment.End,
        crossAlignment: CrossAxisAlignment.Start,
        children: [SizedBox({ height: 50 }), SizedBox({ height: 30 })],
    }),
);
