import { Container, PointerInteract, view } from "builtin:tur/core";

export default view(() =>
    PointerInteract({
        child: Container({ width: 100, height: 50 }),
    }),
);
