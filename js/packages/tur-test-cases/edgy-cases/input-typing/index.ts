import {
    Container,
    view,
    createTextEditingController,
    InputEdgy,
} from "@tur/edgy";

const controller = createTextEditingController({});

export default view(() =>
    Container({
        children: [
            InputEdgy({ controller, fontSize: 14, width: 200, height: 30 }),
        ],
    }),
);
