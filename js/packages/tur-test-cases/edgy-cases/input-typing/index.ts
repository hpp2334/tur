import {
    Container,
    component,
    createTextEditingController,
    InputEdgy,
} from "@tur/edgy";

const controller = createTextEditingController({});

export default component(() =>
    Container({
        children: [
            InputEdgy({ controller, fontSize: 14, width: 200, height: 30 }),
        ],
    }),
);
