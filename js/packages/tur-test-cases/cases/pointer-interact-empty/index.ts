import { mount, PointerInteract, view } from "tur:std";

const App = view(() => PointerInteract({}));

export function start() {
    mount(App);
}
