import { Container, PointerInteract, view } from "@tur/edgy";

export default view(() =>
    PointerInteract({
        child: Container({ width: 100, height: 50 }),
    }),
);
