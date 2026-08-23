import {
    type Brush,
    Column,
    Container,
    CrossAxisAlignment,
    Each,
    Expanded,
    MainAxisSize,
    mount,
    Row,
    SizedBox,
    source,
    Text,
    view,
} from "tur:std";

const seg$ = source<string[]>(["a", "b"]);

const App = view(() =>
    Column({
        crossAlignment: CrossAxisAlignment.Stretch,
        children: [
            Row({
                children: [
                    SizedBox({ width: 16, height: 16 }),
                    Each({
                        items: seg$,
                        mainAxisSize: MainAxisSize.Min,
                        build: (s: string) => Text({ text: s, fontSize: 13 }),
                    }),
                ],
            }),
            SizedBox({ width: 40, height: 10 }),
            Expanded({
                child: Container({ color: 0x00000000 as unknown as Brush }),
            }),
        ],
    }),
);

export function start() {
    mount(App);
}
