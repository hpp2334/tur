import {
    CrossAxisAlignment,
    createStore,
    Expanded,
    Row,
    SizedBox,
    view,
} from "tur:std";

export const store = createStore();

export default view(() =>
    Row({
        crossAlignment: CrossAxisAlignment.Start,
        children: [SizedBox({ width: 50 }), Expanded({ child: SizedBox({}) })],
    }),
);
