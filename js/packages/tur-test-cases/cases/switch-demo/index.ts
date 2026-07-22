import {
    Alignment,
    Color,
    Column,
    Container,
    CrossAxisAlignment,
    derive,
    Expanded,
    get,
    MainAxisAlignment,
    mutate,
    PointerInteract,
    Row,
    Switch,
    source,
    Text,
    view,
} from "tur:std";

const tab$ = source("red");

const TABS = ["red", "green", "blue"] as const;
const HEX: Record<string, string> = {
    red: "#ef4444",
    green: "#22c55e",
    blue: "#3b82f6",
};

function coloredPanel(label: string, hex: string) {
    return Container({
        color: Color.hex(hex),
        alignment: Alignment.Center,
        children: [
            Text({
                text: label,
                fontSize: 36,
                color: Color.hex("#ffffff"),
            }),
        ],
    });
}

export default view(() =>
    Expanded({
        child: Container({
            color: Color.hex("#1a1a2e"),
            padding: 24,
            children: [
                Column({
                    mainAlignment: MainAxisAlignment.Start,
                    crossAlignment: CrossAxisAlignment.Stretch,
                    children: [
                        Row({
                            mainAlignment: MainAxisAlignment.Center,
                            children: TABS.map((c) =>
                                PointerInteract({
                                    onClick: mutate(({ set }, _ev) =>
                                        set(tab$, c),
                                    ),
                                    child: Container({
                                        color: Color.hex(HEX[c]),
                                        padding: 12,
                                        children: [
                                            Text({
                                                text: c,
                                                fontSize: 18,
                                                color: Color.hex("#ffffff"),
                                            }),
                                        ],
                                    }),
                                }),
                            ),
                        }),
                        Container({ height: 16 }),
                        Expanded({
                            child: Switch({
                                value: tab$,
                                cases: [
                                    {
                                        key: "red",
                                        child: () =>
                                            coloredPanel(
                                                "Switch: RED",
                                                HEX.red,
                                            ),
                                    },
                                    {
                                        key: "green",
                                        child: () =>
                                            coloredPanel(
                                                "Switch: GREEN",
                                                HEX.green,
                                            ),
                                    },
                                    {
                                        key: "blue",
                                        child: () =>
                                            coloredPanel(
                                                "Switch: BLUE",
                                                HEX.blue,
                                            ),
                                    },
                                ],
                            }),
                        }),
                        Container({ height: 16 }),
                        Container({
                            height: 60,
                            color: Color.hex("#0f172a"),
                            alignment: Alignment.Center,
                            children: [
                                Text({
                                    text: derive(
                                        () =>
                                            `Switch sees: ${get(tab$).toUpperCase()}`,
                                    ),
                                    fontSize: 20,
                                    color: Color.hex("#e2e8f0"),
                                }),
                            ],
                        }),
                    ],
                }),
            ],
        }),
    }),
);
