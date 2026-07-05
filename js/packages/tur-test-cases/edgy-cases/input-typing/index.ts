import {
    Container,
    createTextEditingController,
    InputEdgy,
    view,
} from "builtin:tur/std";

const controller = createTextEditingController({});

export default view(() =>
    Container({
        children: [
            InputEdgy({ controller, fontSize: 14, width: 200, height: 30 }),
        ],
    }),
);
