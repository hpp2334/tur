import {
    Container,
    createTextEditingController,
    Input,
    mount,
    view,
} from "tur:std";

const controller = createTextEditingController({});

const App = view(() =>
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

export function start() {
    mount(App);
}
