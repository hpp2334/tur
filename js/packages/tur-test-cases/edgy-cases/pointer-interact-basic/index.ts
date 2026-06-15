import { Container, PointerInteract, render } from "@tur/edgy";

render(() =>
    PointerInteract({
        child: Container({ width: 100, height: 50 }),
    }),
);
