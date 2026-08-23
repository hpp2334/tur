import {
    Alignment,
    Column,
    Condition,
    Container,
    CrossAxisAlignment,
    type Element,
    Expanded,
    lifecycleView,
    mount,
    Stack,
    Switch,
    Text,
} from "tur:std";
import { ExplorerScreen } from "./explorer";
import { LandingScreen } from "./landing";
import { hasHttp, repoWatch, view$ } from "./state";
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

/** The viewer component — a plain user-defined component that owns the
 *  `target$` watcher: `lifecycleView` wraps the layout it returns, so
 *  `repoWatch.start$` fires when the component mounts and
 *  `repoWatch.stop$` when it is torn down (case switch / recompile /
 *  module reload). This is the general user shape — any component that
 *  owns side-effecting resources (watchers, subscriptions) ties their
 *  lifetime to its own subtree this way, from inside the component. */
function GithubViewer(): Element {
    return lifecycleView(() => ({
        element: Expanded({
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
        onMounted$: repoWatch.start$,
        beforeDestroy$: repoWatch.stop$,
    }));
}

/** The module entrypoint — mounts the component, like any other view.
 *  (In the playground the compiler intercepts `mount` to publish the view
 *  into the viewer pane; elsewhere this runs the viewer as a root module.) */
export function start() {
    mount(GithubViewer());
}
