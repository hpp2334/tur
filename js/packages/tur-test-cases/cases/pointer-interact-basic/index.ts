import { Container, mount, PointerInteract, view } from "tur:std";

const App = view(() =>
    PointerInteract().child(Container().width(100).height(50).build()).build(),
);

export function start() {
    mount(App);
}
