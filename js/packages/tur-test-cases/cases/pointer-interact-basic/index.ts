import { Container, createStore, PointerInteract, view } from "tur:std";

export const store = createStore();

export default view(() =>
    PointerInteract({
        child: Container({ width: 100, height: 50 }),
    }),
);
