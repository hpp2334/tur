import { mount, PointerInteract, view } from "tur:std";

const App = view(() => PointerInteract().build());

export function start() {
    mount(App);
}
