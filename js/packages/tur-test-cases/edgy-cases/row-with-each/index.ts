import {
    Column,
    Container,
    CrossAxisAlignment,
    view,
    Each,
    Expanded,
    MainAxisSize,
    Row,
    SizedBox,
    source,
    Text,
} from "@tur/edgy";

const seg$ = source<string[]>(["a", "b"]);

export default view(() =>
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
                child: Container({ color: 0x00000000 }),
            }),
        ],
    }),
);
