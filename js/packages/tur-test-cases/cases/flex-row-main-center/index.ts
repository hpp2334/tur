import {
    CrossAxisAlignment,
    createStore,
    MainAxisAlignment,
    Row,
    SizedBox,
    view,
} from "tur:std";

export const store = createStore();

export default view(() =>
    Row({
        mainAlignment: MainAxisAlignment.Center,
        crossAlignment: CrossAxisAlignment.Start,
        children: [SizedBox({ width: 50 }), SizedBox({ width: 30 })],
    }),
);
