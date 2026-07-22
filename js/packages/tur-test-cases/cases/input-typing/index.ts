import {
    Container,
    createTextEditingController,
    Input,
    view,
} from "tur:std";

const controller = createTextEditingController({});

export default view(() =>
    Container({
        children: [Input({ controller, fontSize: 14, width: 200, height: 30 })],
    }),
);
