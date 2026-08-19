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
        children: [
            Input({
                controller,
                multiline: true,
                fontSize: 14,
                width: 300,
                height: 400,
            }),
        ],
    }),
);
