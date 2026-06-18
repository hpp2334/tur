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
