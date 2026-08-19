import {
    Alignment,
    Column,
    Condition,
    Container,
    CrossAxisAlignment,
    type Element,
    Expanded,
    lifecycleView,
    Stack,
    Switch,
    Text,
} from "tur:std";
import { ExplorerScreen } from "./explorer";
import { LandingScreen } from "./landing";
import { hasHttp, repoWatch, view$ } from "./state";
import { COLORS } from "./theme";

// The in-realm case compiler (`compile.ts` → `runCaseBody`) injects
// `__setCaseView` as a function parameter when evaluating this module.
declare const __setCaseView: (view: unknown) => void;

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

function CaseRoot(): Element {
    return Expanded({
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
    });
}

/** Advanced case: an explicit `start()` (wins over the default-export
 *  wrapper). The root is wrapped in `lifecycleView` so the `target$` watcher
 *  (`repoWatch`) starts when this case's view mounts and stops when it is
 *  torn down (case switch / recompile / module reload) — the watcher lives
 *  exactly as long as the tree that owns it. */
export function start() {
    __setCaseView(
        lifecycleView(() => ({
            element: CaseRoot(),
            onMounted$: repoWatch.start$,
            beforeDestroy$: repoWatch.stop$,
        })),
    );
}
