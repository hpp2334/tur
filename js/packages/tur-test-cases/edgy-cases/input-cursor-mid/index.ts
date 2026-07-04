import {
    Container,
    createTextEditingController,
    InputEdgy,
    view,
} from "builtin:tur/core";

const ctrl = createTextEditingController({});

(globalThis as Record<string, unknown>).__setCursorMidTick = () => {};

export default view(() =>
    Container({
        children: [
            InputEdgy({
                controller: ctrl,
                fontSize: 14,
                width: 200,
                height: 30,
            }),
        ],
    }),
);
