import {
    Container,
    createTextEditingController,
    InputEdgy,
    view,
} from "builtin:tur/core";

const controller = createTextEditingController({});

export default view(() =>
    Container({
        children: [
            InputEdgy({
                controller,
                multiline: true,
                fontSize: 14,
                width: 300,
                height: 400,
            }),
        ],
    }),
);
