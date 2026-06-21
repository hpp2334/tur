import {
    Alignment,
    Axis,
    Color,
    Column,
    Container,
    CrossAxisAlignment,
    component,
    derive,
    Expanded,
    get,
    LazyList,
    MainAxisAlignment,
    MainAxisSize,
    Row,
    SizedBox,
    source,
    Text,
} from "@tur/edgy";

// ---------------------------------------------------------------------------
// "10,000-item Contact List" — a demo of tur's (now actually lazy) LazyList.
//
// Renders 10,000 contact rows with fixed 56-pixel item extent. Only the
// visible range + overscan is mounted at any time (~16 of 10,000). Scroll
// smoothly — that's the proof virtualization is working.
// ---------------------------------------------------------------------------

const ITEM_COUNT = 10000;
const ITEM_HEIGHT = 56;

// Track scroll progress to show a "you are here" indicator. The percent
// updates as the user scrolls.
const scrollPct$ = source(0);

// Deterministic name generator — keeps the bundle tiny and the demo stable.
const FIRST_NAMES = [
    "Asha",
    "Bryn",
    "Cai",
    "Dara",
    "Eva",
    "Finn",
    "Gita",
    "Hugo",
    "Iris",
    "Jin",
    "Kai",
    "Lior",
    "Mira",
    "Noa",
    "Olin",
    "Pia",
    "Quinn",
    "Ren",
    "Suri",
    "Theo",
    "Uma",
    "Vera",
    "Wim",
    "Xan",
    "Yara",
    "Zane",
];
const LAST_NAMES = [
    "Albright",
    "Bauer",
    "Cohen",
    "Dixon",
    "Eng",
    "Frost",
    "Grant",
    "Hayes",
    "Iyer",
    "Jang",
    "Kerr",
    "Lin",
    "Mehta",
    "Novak",
    "Okafor",
    "Patel",
    "Quinn",
    "Reyes",
    "Shin",
    "Tran",
    "Ueda",
    "Vargas",
    "Wong",
    "Xu",
    "Yoon",
    "Zhou",
];

function nameFor(index: number): string {
    const fn = FIRST_NAMES[index % FIRST_NAMES.length];
    const ln = LAST_NAMES[(index * 7) % LAST_NAMES.length];
    return `${fn} ${ln}`;
}

function initialsFor(name: string): string {
    return name
        .split(" ")
        .map((p) => p[0])
        .join("");
}

function hueFor(index: number): number {
    return (index * 47) % 360;
}

function hslToHex(h: number, s: number, l: number): string {
    const sat = s / 100;
    const light = l / 100;
    const c = (1 - Math.abs(2 * light - 1)) * sat;
    const hp = h / 60;
    const x = c * (1 - Math.abs((hp % 2) - 1));
    let r1 = 0,
        g1 = 0,
        b1 = 0;
    if (hp < 1) {
        r1 = c;
        g1 = x;
    } else if (hp < 2) {
        r1 = x;
        g1 = c;
    } else if (hp < 3) {
        g1 = c;
        b1 = x;
    } else if (hp < 4) {
        g1 = x;
        b1 = c;
    } else if (hp < 5) {
        r1 = x;
        b1 = c;
    } else {
        r1 = c;
        b1 = x;
    }
    const m = light - c / 2;
    const r = Math.round((r1 + m) * 255);
    const g = Math.round((g1 + m) * 255);
    const b = Math.round((b1 + m) * 255);
    return `#${r.toString(16).padStart(2, "0")}${g.toString(16).padStart(2, "0")}${b.toString(16).padStart(2, "0")}`;
}

function buildRow(index: number): unknown {
    const name = nameFor(index);
    const initials = initialsFor(name);
    const hue = hueFor(index);

    return Container({
        height: ITEM_HEIGHT,
        padding: 12,
        color: index % 2 === 0 ? Color.hex("#ffffff") : Color.hex("#f8fafc"),
        children: [
            Row({
                crossAlignment: CrossAxisAlignment.Center,
                children: [
                    Container({
                        width: 36,
                        height: 36,
                        borderRadius: 999,
                        color: Color.hex(hslToHex(hue, 60, 55)),
                        alignment: Alignment.Center,
                        children: [
                            Text({
                                text: initials,
                                fontSize: 13,
                                color: Color.hex("#ffffff"),
                            }),
                        ],
                    }),
                    SizedBox({ width: 12 }),
                    Expanded({
                        child: Column({
                            mainAlignment: MainAxisAlignment.Center,
                            crossAlignment: CrossAxisAlignment.Start,
                            mainAxisSize: MainAxisSize.Min,
                            children: [
                                Text({
                                    text: name,
                                    fontSize: 13,
                                    color: Color.hex("#0f172a"),
                                }),
                                SizedBox({ height: 2 }),
                                Text({
                                    text: `Item #${index + 1} of ${ITEM_COUNT}`,
                                    fontSize: 11,
                                    color: Color.hex("#64748b"),
                                }),
                            ],
                        }),
                    }),
                ],
            }),
        ],
    });
}

function Header(): unknown {
    return Container({
        height: 56,
        padding: 12,
        color: Color.hex("#1e293b"),
        children: [
            Row({
                crossAlignment: CrossAxisAlignment.Center,
                children: [
                    Text({
                        text: "Contacts (10,000 items, ~16 mounted)",
                        fontSize: 14,
                        color: Color.hex("#ffffff"),
                    }),
                    Expanded({ child: SizedBox({ height: 1 }) }),
                    Text({
                        text: derive(
                            () => `Scroll: ${Math.round(get(scrollPct$))}%`,
                        ),
                        fontSize: 11,
                        color: Color.hex("#94a3b8"),
                    }),
                ],
            }),
        ],
    });
}

export default component(() =>
    Expanded({
        child: Container({
            color: Color.hex("#ffffff"),
            children: [
                Column({
                    crossAlignment: CrossAxisAlignment.Stretch,
                    children: [
                        Header(),
                        Expanded({
                            child: LazyList({
                                axis: Axis.Vertical,
                                itemCount: ITEM_COUNT,
                                itemExtent: ITEM_HEIGHT,
                                overscan: 4,
                                builder: buildRow,
                            }),
                        }),
                    ],
                }),
            ],
        }),
    }),
);
