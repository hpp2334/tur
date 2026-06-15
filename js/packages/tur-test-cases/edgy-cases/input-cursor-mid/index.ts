import {
    Container,
    createTextEditingController,
    InputEdgy,
    render,
} from "@tur/edgy";

const ctrl = createTextEditingController({});

(globalThis as Record<string, unknown>).__setCursorMidTick = () => {};

render(() =>
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
