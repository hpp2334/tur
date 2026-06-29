import { Column, component, MainAxisSize, SizedBox } from "@tur/edgy";

// Three 300px children in a Min column — total 900px overflows the 600px
// viewport. Children should keep their natural height (not squish to 0).
export default component(() =>
    Column({
        mainAxisSize: MainAxisSize.Min,
        children: [
            SizedBox({ height: 300 }),
            SizedBox({ height: 300 }),
            SizedBox({ height: 300 }),
        ],
    }),
);
