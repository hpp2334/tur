import { Container, component, PointerInteract } from "@tur/edgy";

export default component(() =>
    PointerInteract({
        child: Container({ width: 100, height: 50 }),
    }),
);
