import {
    Container,
    createTextEditingController,
    Input,
    mount,
    view,
} from "tur:std";

const controller = createTextEditingController({});

const App = view(() =>
    Container()
        .children([
            Input()
                .controller(controller)
                .multiline(true)
                .fontSize(14)
                .width(300)
                .height(400)
                .build(),
        ])
        .build(),
);

export function start() {
    mount(App);
}
