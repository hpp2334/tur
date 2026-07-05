import { Container, PointerInteract, view } from "builtin:tur/std";

export default view(() =>
    PointerInteract({
        child: Container({ width: 100, height: 50 }),
    }),
);
