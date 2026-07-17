import {
    Container,
    createTextEditingController,
    Input,
    view,
} from "builtin:tur/std";

const controller = createTextEditingController({});

export default view(() =>
    Container({
        children: [
            Input({
                controller,
                multiline: true,
                fontSize: 14,
                width: 300,
                height: 400,
            }),
        ],
    }),
);
