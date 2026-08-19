import {
    Column,
    CrossAxisAlignment,
    createStore,
    Expanded,
    SizedBox,
    view,
} from "tur:std";

export const store = createStore();

export default view(() =>
    Column({
        crossAlignment: CrossAxisAlignment.Start,
        children: [SizedBox({ height: 50 }), Expanded({ child: SizedBox({}) })],
    }),
);
