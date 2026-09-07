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
                .fontSize(14)
                .width(200)
                .height(30)
                .build(),
        ])
        .build(),
);

export function start() {
    mount(App);
}
