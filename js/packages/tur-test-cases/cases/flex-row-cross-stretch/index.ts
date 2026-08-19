import { CrossAxisAlignment, createStore, Row, SizedBox, view } from "tur:std";

export const store = createStore();

export default view(() =>
    Row({
        crossAlignment: CrossAxisAlignment.Stretch,
        children: [SizedBox({ width: 50 }), SizedBox({ width: 30 })],
    }),
);
