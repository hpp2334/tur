import {
    Container,
    createTextEditingController,
    Input,
    view,
} from "builtin:tur/std";

const ctrl = createTextEditingController({});

(globalThis as Record<string, unknown>).__setCursorMidTick = () => {};

export default view(() =>
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
