import {
    Alignment,
    Column,
    Condition,
    Container,
    CrossAxisAlignment,
    derive,
    Each,
    type EdgyElement,
    Expanded,
    get,
    ImageEdgy,
    MainAxisSize,
    MouseRegion,
    mutate,
    PointerInteract,
    Row,
    SizedBox,
    type StoreCtx,
    Text,
} from "@tur/edgy";
import {
    connect,
    getIcon,
    openAddServer,
    openEditServer,
    removeServer,
    servers$,
    type WebDavServer,
} from "./state";
import { COLORS } from "./theme";
import { Button } from "./ui";

// --- Server list screen ---------------------------------------------------

export function ServerListScreen(): EdgyElement {
    return Column({
        crossAlignment: CrossAxisAlignment.Stretch,
        children: [
            Row({
                crossAlignment: CrossAxisAlignment.Center,
                children: [
                    Column({
                        crossAlignment: CrossAxisAlignment.Start,
                        mainAxisSize: MainAxisSize.Min,
                        children: [
                            Text({
                                text: "WebDAV Servers",
                                fontSize: 24,
                                color: COLORS.text,
                            }),
                            SizedBox({ height: 4 }),
                            Text({
                                text: derive(() => {
                                    const n = get(servers$).length;
                                    return `${n} server${n === 1 ? "" : "s"} registered`;
                                }),
                                fontSize: 13,
                                color: COLORS.textMuted,
                            }),
                        ],
                    }),
                    AddServerButton(),
                ],
            }),
            SizedBox({ height: 18 }),
            Expanded({
                child: Container({
                    color: COLORS.pageBg,
                    children: [
                        Condition({
                            condition: derive(() => get(servers$).length === 0),
                            child: () => EmptyState(),
                            elseChild: () =>
                                Column({
                                    crossAlignment: CrossAxisAlignment.Stretch,
                                    mainAxisSize: MainAxisSize.Min,
                                    children: [
                                        Each({
                                            items: servers$,
                                            crossAlignment:
                                                CrossAxisAlignment.Stretch,
                                            build: (
                                                _s: WebDavServer,
                                                i: number,
                                            ) =>
                                                Column({
                                                    crossAlignment:
                                                        CrossAxisAlignment.Stretch,
                                                    mainAxisSize:
                                                        MainAxisSize.Min,
                                                    children: [
                                                        i === 0
                                                            ? SizedBox({
                                                                  width: 0,
                                                                  height: 0,
                                                              })
                                                            : SizedBox({
                                                                  height: 10,
                                                              }),
                                                        ServerCard({
                                                            server: _s,
                                                        }),
                                                    ],
                                                }),
                                        }),
                                    ],
                                }),
                        }),
                    ],
                }),
            }),
        ],
    });
}

function AddServerButton(): EdgyElement {
    return MouseRegion({
        cursor: "pointer",
        child: PointerInteract({
            onClick: mutate((_ctx: StoreCtx, _ev) => openAddServer()),
            child: Container({
                padding: 10,
                borderRadius: 8,
                color: COLORS.accent,
                children: [
                    Row({
                        mainAxisSize: MainAxisSize.Min,
                        children: [
                            ImageEdgy({
                                resourceId: getIcon("plus"),
                                width: 13,
                                height: 13,
                                queryKey: ["plus-icon"],
                            }),
                            SizedBox({ width: 7 }),
                            Text({
                                text: "Add Server",
                                fontSize: 13,
                                color: COLORS.accentFg,
                            }),
                        ],
                    }),
                ],
            }),
        }),
    });
}

function EmptyState(): EdgyElement {
    return Container({
        padding: 40,
        alignment: Alignment.Center,
        children: [
            Column({
                crossAlignment: CrossAxisAlignment.Center,
                mainAxisSize: MainAxisSize.Min,
                children: [
                    ImageEdgy({
                        resourceId: getIcon("folder"),
                        width: 48,
                        height: 48,
                        queryKey: ["empty-icon"],
                    }),
                    SizedBox({ height: 14 }),
                    Text({
                        text: "No servers yet",
                        fontSize: 15,
                        color: COLORS.text,
                    }),
                    SizedBox({ height: 4 }),
                    Text({
                        text: "Add a WebDAV server to browse its files.",
                        fontSize: 13,
                        color: COLORS.textMuted,
                    }),
                ],
            }),
        ],
    });
}

// `Each` rebuilds this card whenever `servers$` changes (add/edit/remove), so
// the props can be plain captured values rather than reactive reads.
function ServerCard({ server }: { server: WebDavServer }): EdgyElement {
    return Container({
        borderRadius: 10,
        padding: 14,
        color: COLORS.panel,
        borderColor: COLORS.border,
        borderWidth: 1,
        children: [
            Row({
                children: [
                    ImageEdgy({
                        resourceId: getIcon("folder"),
                        width: 22,
                        height: 22,
                        queryKey: ["card-icon"],
                    }),
                    SizedBox({ width: 12 }),
                    Expanded({
                        child: Column({
                            crossAlignment: CrossAxisAlignment.Start,
                            mainAxisSize: MainAxisSize.Min,
                            children: [
                                Text({
                                    text: server.name,
                                    fontSize: 15,
                                    color: COLORS.text,
                                }),
                                SizedBox({ height: 3 }),
                                Row({
                                    mainAxisSize: MainAxisSize.Min,
                                    children: [
                                        Text({
                                            text: server.url,
                                            fontSize: 12,
                                            color: COLORS.textMuted,
                                        }),
                                        SizedBox({ width: 8 }),
                                        Container({
                                            padding: 3,
                                            borderRadius: 5,
                                            color: COLORS.badgeBg,
                                            children: [
                                                Text({
                                                    text: "Basic",
                                                    fontSize: 10,
                                                    color: COLORS.badgeFg,
                                                }),
                                            ],
                                        }),
                                    ],
                                }),
                            ],
                        }),
                    }),
                    SizedBox({ width: 10 }),
                    Button({
                        label: "Connect",
                        bg: COLORS.accent,
                        fg: COLORS.accentFg,
                        onClick: () => connect(server),
                        padding: 7,
                    }),
                    SizedBox({ width: 6 }),
                    Button({
                        label: "Edit",
                        bg: COLORS.subtleButton,
                        fg: COLORS.subtleButtonFg,
                        onClick: () => openEditServer(server),
                        padding: 7,
                    }),
                    SizedBox({ width: 6 }),
                    Button({
                        label: "Remove",
                        bg: COLORS.subtleButton,
                        fg: COLORS.danger,
                        onClick: () => removeServer(server),
                        padding: 7,
                    }),
                ],
            }),
        ],
    });
}
