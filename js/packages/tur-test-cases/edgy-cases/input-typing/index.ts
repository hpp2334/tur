import {
    Container,
    createTextEditingController,
    InputEdgy,
    render,
} from "@tur/edgy";

const controller = createTextEditingController({});

render(() =>
    Container({
        children: [
            InputEdgy({ controller, fontSize: 14, width: 200, height: 30 }),
        ],
    }),
);
