import {
    Column,
    Container,
    CrossAxisAlignment,
    PointerInteract,
    view,
} from "tur:std";

export default view(() =>
    Column({
        crossAlignment: CrossAxisAlignment.Start,
        children: [
            PointerInteract({ child: Container({ width: 80, height: 40 }) }),
            PointerInteract({ child: Container({ width: 60, height: 30 }) }),
        ],
    }),
);
