import {
    Alignment,
    Column,
    Condition,
    Container,
    CrossAxisAlignment,
    component,
    derive,
    type EdgyElement,
    Expanded,
    get,
    Positioned,
    Stack,
    Switch,
    Text,
} from "@tur/edgy";
import { ConfirmDeleteDialog, ConnectDialog, NewFolderDialog } from "./dialogs";
import { ExplorerScreen } from "./explorer";
import { ServerListScreen } from "./servers";
import {
    confirmDelete$,
    connectOpen$,
    hasHttp,
    newFolderOpen$,
    view$,
} from "./state";
import { COLORS } from "./theme";

/** Capability guard: under the native engine `__tur.request` is absent, so the
 *  whole client is replaced with a short notice. */
function Unsupported(): EdgyElement {
    return Container({
        padding: 40,
        alignment: Alignment.Center,
        children: [
            Text({
                text: "The WebDAV client needs the browser playground (HTTP + file IO are wasm-only).",
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
            { key: "list", child: () => ServerListScreen() },
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
                Condition({
                    condition: connectOpen$,
                    child: () =>
                        Positioned({
                            top: 0,
                            left: 0,
                            right: 0,
                            bottom: 0,
                            child: ConnectDialog(),
                        }),
                }),
                Condition({
                    condition: newFolderOpen$,
                    child: () =>
                        Positioned({
                            top: 0,
                            left: 0,
                            right: 0,
                            bottom: 0,
                            child: NewFolderDialog(),
                        }),
                }),
                Condition({
                    condition: derive(() => get(confirmDelete$) !== null),
                    child: () =>
                        Positioned({
                            top: 0,
                            left: 0,
                            right: 0,
                            bottom: 0,
                            child: ConfirmDeleteDialog(),
                        }),
                }),
            ],
        }),
    }),
);
