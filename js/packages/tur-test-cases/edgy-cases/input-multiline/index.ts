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
