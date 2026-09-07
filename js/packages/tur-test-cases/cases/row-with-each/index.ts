import {
    type Brush,
    Column,
    Container,
    CrossAxisAlignment,
    Each,
    Expanded,
    mount,
    Row,
    SizedBox,
    source,
    Text,
    view,
} from "tur:std";

const App = view(() => {
    // Local state: the view fn runs exactly once (at build), so this atom is
    // stable for the life of the tree. NOTE the rebuild boundary: `Each.build`
    // re-runs for every item whenever `items` changes — keep atoms OUT of the
    // build thunk (state there would be re-created on every items change).
    const seg$ = source<string[]>(["a", "b"]);

    return Column()
        .crossAlignment(CrossAxisAlignment.Stretch)
        .children([
            Row()
                .children([
                    SizedBox().width(16).height(16).build(),
                    Each({ items: seg$ })
                        .itemBuilder((s: string) =>
                            Text({ text: s }).fontSize(13).build(),
                        )
                        .build(),
                ])
                .build(),
            SizedBox().width(40).height(10).build(),
            Expanded()
                .child(
                    Container()
                        .color(0x00000000 as unknown as Brush)
                        .build(),
                )
                .build(),
        ])
        .build();
});

export function start() {
    mount(App);
}
