import {
    Container,
    createTextEditingController,
    Input,
    mount,
    view,
} from "tur:std";

const ctrl = createTextEditingController({});

(globalThis as Record<string, unknown>).__setCursorMidTick = () => {};

const App = view(() =>
    Container({
        children: [
            Input({
                controller: ctrl,
                fontSize: 14,
                width: 200,
                height: 30,
            }),
        ],
    }),
);

export function start() {
    mount(App);
}
