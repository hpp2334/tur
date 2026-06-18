import { Column, CrossAxisAlignment, component, SizedBox } from "@tur/edgy";

export default component(() =>
    Column({
        crossAlignment: CrossAxisAlignment.Start,
        children: [SizedBox({ width: 100, height: 50 })],
    }),
);
