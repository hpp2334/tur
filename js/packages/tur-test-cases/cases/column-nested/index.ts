import { Column, CrossAxisAlignment, mount, SizedBox, view } from "tur:std";

const App = view(() =>
    Column()
        .crossAlignment(CrossAxisAlignment.Start)
        .children([
            SizedBox().height(50).build(),
            // No `mainAxisSize`: the default (Max) must degenerate to content
            // size because the parent Column passes UNBOUNDED main-axis
            // constraints to non-flex children (Flutter RenderFlex parity).
            Column()
                .crossAlignment(CrossAxisAlignment.Start)
                .children([SizedBox().height(30).build()])
                .build(),
            SizedBox().height(40).build(),
        ])
        .build(),
);

export function start() {
    mount(App);
}
