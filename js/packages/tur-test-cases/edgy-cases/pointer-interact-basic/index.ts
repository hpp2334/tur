import { Container, view, PointerInteract } from "@tur/edgy";

export default view(() =>
    PointerInteract({
        child: Container({ width: 100, height: 50 }),
    }),
);
