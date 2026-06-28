import {
    Alignment,
    Column,
    Condition,
    Container,
    CrossAxisAlignment,
    component,
    type EdgyElement,
    Expanded,
    Stack,
    Switch,
    Text,
} from "@tur/edgy";
import { ExplorerScreen } from "./explorer";
import { LandingScreen } from "./landing";
import { hasHttp, view$ } from "./state";
import { COLORS } from "./theme";

/** Capability guard: under the native engine `__tur.request` is absent, so the
 *  whole viewer is replaced with a short notice. */
function Unsupported(): EdgyElement {
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

function Body(): EdgyElement {
    return Switch({
        value: view$,
        cases: [
            { key: "landing", child: () => LandingScreen() },
            { key: "explorer", child: () => ExplorerScreen() },
        ],
    });
}

export default component(() =>
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
