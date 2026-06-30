import { view, Positioned, SizedBox, Stack } from "@tur/edgy";

export default view(() =>
    Stack({
        children: [
            Positioned({
                left: 10,
                top: 20,
                child: SizedBox({ width: 50, height: 50 }),
            }),
        ],
    }),
);
