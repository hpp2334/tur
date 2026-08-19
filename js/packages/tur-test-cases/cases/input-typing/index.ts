import {
    Container,
    createStore,
    createTextEditingController,
    Input,
    view,
} from "tur:std";

export const store = createStore();

const controller = createTextEditingController({});

export default view(() =>
    Container({
        children: [Input({ controller, fontSize: 14, width: 200, height: 30 })],
    }),
);
