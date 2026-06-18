import { component, SizedBox, Stack } from "@tur/edgy";

export default component(() =>
    Stack({
        children: [
            SizedBox({ width: 100, height: 100 }),
            SizedBox({ width: 200, height: 200 }),
        ],
    }),
);
