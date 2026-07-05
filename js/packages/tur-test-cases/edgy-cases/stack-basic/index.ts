import { SizedBox, Stack, view } from "builtin:tur/std";

export default view(() =>
    Stack({
        children: [
            SizedBox({ width: 100, height: 100 }),
            SizedBox({ width: 200, height: 200 }),
        ],
    }),
);
