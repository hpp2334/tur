import {
    Alignment,
    Column,
    Condition,
    Container,
    CrossAxisAlignment,
    type Element,
    Expanded,
    Stack,
    Switch,
    Text,
    view,
} from "builtin:tur/std";
import { ExplorerScreen } from "./explorer";
import { LandingScreen } from "./landing";
import { hasHttp, view$ } from "./state";
import { COLORS } from "./theme";

/** Capability guard: the viewer needs the playground's HTTP module; if it is
 *  somehow absent the whole viewer is replaced with a short notice. */
function Unsupported(): Element {
    return Container({
        padding: 40,
        alignment: Alignment.Center,
        children: [
            Text({
                text: "The GitHub viewer needs the browser playground (HTTP + file IO are wasm-only).",
                fontSize: 14,
                color: COLORS.textMuted,
            }),
        ],
    });
}

function Body(): Element {
    return Switch({
        value: view$,
        cases: [
            { key: "landing", child: () => LandingScreen() },
            { key: "explorer", child: () => ExplorerScreen() },
        ],
    });
}

export default view(() =>
    Expanded({
        child: Stack({
            children: [
                Container({
                    color: COLORS.pageBg,
                    padding: 22,
                    children: [
                        Column({
                            crossAlignment: CrossAxisAlignment.Stretch,
                            children: [
                                Condition({
                                    condition: hasHttp,
                                    child: () => Body(),
                                    elseChild: () => Unsupported(),
                                }),
                            ],
                        }),
                    ],
                }),
            ],
        }),
    }),
);
