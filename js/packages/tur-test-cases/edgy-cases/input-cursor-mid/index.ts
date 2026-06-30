import {
    Container,
    view,
    createTextEditingController,
    InputEdgy,
} from "@tur/edgy";

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
