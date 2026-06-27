import {
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
    MainAxisAlignment,
    MainAxisSize,
    MouseRegion,
    mutate,
    PointerInteract,
    Row,
    SizedBox,
    type StoreCtx,
    Switch,
    Text,
} from "@tur/edgy";
import {
    type DirEntry,
    disconnect,
    doDownload,
    doUpload,
    entries$,
    error$,
    fmtSize,
    getIcon,
    loading$,
    navigateTo,
    navigateToRoot,
    openFolder,
    openNewFolder,
    pathSegments$,
    refresh,
    requestDelete,
    selectEntry,
    selectedEntry$,
    selectedHref$,
} from "./state";
import { COLORS } from "./theme";
import { Button, IconButton } from "./ui";

// --- File row -------------------------------------------------------------

function FileRow({
    entry,
    index,
}: {
    entry: DirEntry;
    index: number;
}): EdgyElement {
    return Column({
        crossAlignment: CrossAxisAlignment.Stretch,
        mainAxisSize: MainAxisSize.Min,
        children: [
            index === 0
                ? SizedBox({ width: 0, height: 0 })
                : SizedBox({ height: 4 }),
            MouseRegion({
                cursor: "pointer",
                child: PointerInteract({
                    onClick: mutate((_ctx: StoreCtx, _ev) => {
                        if (entry.isDir) openFolder(entry);
                        else selectEntry(entry);
                    }),
                    child: Container({
                        padding: 10,
                        borderRadius: 8,
                        color: derive(() =>
                            get(selectedHref$) === entry.href
                                ? COLORS.rowSelected
                                : COLORS.rowHover,
                        ),
                        children: [
                            Row({
                                children: [
                                    ImageEdgy({
                                        resourceId: entry.isDir
                                            ? getIcon("folder")
                                            : getIcon("file"),
                                        width: 18,
                                        height: 18,
                                        queryKey: ["row-icon"],
                                    }),
                                    SizedBox({ width: 10 }),
                                    Expanded({
                                        child: Text({
                                            text: entry.name,
                                            fontSize: 13,
                                            color: COLORS.text,
                                        }),
                                    }),
                                    SizedBox({ width: 10 }),
                                    Text({
                                        text: entry.isDir
                                            ? "Folder"
                                            : fmtSize(entry.size),
                                        fontSize: 11,
                                        color: COLORS.textMuted,
                                    }),
                                ],
                            }),
                        ],
                    }),
                }),
            }),
        ],
    });
}

// --- Breadcrumb -----------------------------------------------------------

function Crumb({
    label,
    index,
}: {
    label: string;
    index: number;
}): EdgyElement {
    // index === -1 means the server root.
    return MouseRegion({
        cursor: "pointer",
        child: PointerInteract({
            onClick: mutate((_ctx: StoreCtx, _ev) => {
                if (index === -1) navigateToRoot();
                else navigateTo(index);
            }),
            child: Container({
                padding: 4,
                children: [
                    Text({ text: label, fontSize: 13, color: COLORS.accent }),
                ],
            }),
        }),
    });
}

export function Breadcrumb(): EdgyElement {
    // Retained for API completeness; the explorer inlines its own breadcrumb
    // (a `Crumb("Root")` + a reactive path `Text`) because an `Each`-built
    // breadcrumb inside the top-bar Row would inflate it under the engine's
    // flex layout.
    return Crumb({ label: "Root", index: -1 });
}

// --- Explorer screen ------------------------------------------------------

export function ExplorerScreen(): EdgyElement {
    // NOTE: a `Container` used as a direct child of this Column would fill the
    // Column's (tight) main-axis constraint and starve the `Expanded` file
    // list to zero height. `Row`/`Column` children resist that and size to
    // their content, so the top bar and toolbar are bare `Row`s (the panel
    // styling is applied per-button instead of via a wrapping Container).
    return Column({
        crossAlignment: CrossAxisAlignment.Stretch,
        children: [
            // Top bar: back + breadcrumb + disconnect. The crumbs are inlined
            // directly into this Row (via the transparent `Each`+`Fragment`)
            // rather than wrapped in a nested `Row` — a nested flex here
            // inflates to the Column's full height and starves the file list.
            Row({
                children: [
                    IconButton({
                        resourceId: getIcon("back"),
                        onClick: (_ctx) => navigateToRoot(),
                    }),
                    SizedBox({ width: 8 }),
                    Crumb({ label: "Root", index: -1 }),
                    // Path segments as a single reactive Text. (We avoid `Each`
                    // here: an `Each` inside this content-sized Row inflates it
                    // to the parent Column's full height and starves the file
                    // list. Per-segment click is sacrificed; the back arrow
                    // and "Root" crumb still navigate up.)
                    Text({
                        text: derive(() => {
                            const segs = get(pathSegments$);
                            return segs.length ? ` / ${segs.join(" / ")}` : "";
                        }),
                        fontSize: 13,
                        color: COLORS.textMuted,
                    }),
                    Expanded({ child: SizedBox({ width: 0, height: 0 }) }),
                    Button({
                        label: "Disconnect",
                        bg: COLORS.subtleButton,
                        fg: COLORS.subtleButtonFg,
                        onClick: disconnect,
                        padding: 7,
                    }),
                ],
            }),
            SizedBox({ height: 6 }),
            // Toolbar.
            Row({
                children: [
                    IconButton({
                        resourceId: getIcon("refresh"),
                        onClick: refresh,
                    }),
                    SizedBox({ width: 6 }),
                    Button({
                        label: "New Folder",
                        bg: COLORS.subtleButton,
                        fg: COLORS.subtleButtonFg,
                        onClick: openNewFolder,
                        padding: 7,
                    }),
                    SizedBox({ width: 6 }),
                    Button({
                        label: "Upload",
                        bg: COLORS.subtleButton,
                        fg: COLORS.subtleButtonFg,
                        onClick: doUpload,
                        padding: 7,
                    }),
                    SizedBox({ width: 6 }),
                    Button({
                        label: "Download",
                        bg: COLORS.subtleButton,
                        fg: COLORS.subtleButtonFg,
                        onClick: (_ctx) => {
                            const e = get(selectedEntry$);
                            if (e && !e.isDir) doDownload();
                        },
                        padding: 7,
                    }),
                    SizedBox({ width: 6 }),
                    Button({
                        label: "Delete",
                        bg: COLORS.subtleButton,
                        fg: COLORS.danger,
                        onClick: (_ctx) => {
                            const e = get(selectedEntry$);
                            if (e) requestDelete(e);
                        },
                        padding: 7,
                    }),
                ],
            }),
            SizedBox({ height: 6 }),
            // Error banner.
            Condition({
                condition: derive(() => get(error$) !== null),
                child: () =>
                    Row({
                        mainAxisSize: MainAxisSize.Min,
                        children: [
                            Container({
                                padding: 8,
                                color: COLORS.errorBg,
                                children: [
                                    Text({
                                        text: derive(() => get(error$) ?? ""),
                                        fontSize: 12,
                                        color: COLORS.danger,
                                    }),
                                ],
                            }),
                        ],
                    }),
                elseChild: () => SizedBox({ width: 0, height: 0 }),
            }),
            // File list (loading / empty / list).
            Expanded({
                child: Container({
                    color: COLORS.pageBg,
                    padding: 10,
                    children: [
                        Switch({
                            value: derive(() => {
                                if (get(loading$) && get(entries$).length === 0)
                                    return "loading";
                                if (get(entries$).length === 0) return "empty";
                                return "list";
                            }),
                            cases: [
                                {
                                    key: "loading",
                                    child: () =>
                                        Text({
                                            text: "Loading…",
                                            fontSize: 13,
                                            color: COLORS.textMuted,
                                        }),
                                },
                                {
                                    key: "empty",
                                    child: () =>
                                        Text({
                                            text: "This folder is empty.",
                                            fontSize: 13,
                                            color: COLORS.textMuted,
                                        }),
                                },
                                {
                                    key: "list",
                                    child: () =>
                                        Column({
                                            crossAlignment:
                                                CrossAxisAlignment.Stretch,
                                            mainAxisSize: MainAxisSize.Min,
                                            children: [
                                                Each({
                                                    items: entries$,
                                                    crossAlignment:
                                                        CrossAxisAlignment.Stretch,
                                                    build: (
                                                        e: DirEntry,
                                                        i: number,
                                                    ) =>
                                                        FileRow({
                                                            entry: e,
                                                            index: i,
                                                        }),
                                                }),
                                            ],
                                        }),
                                },
                            ],
                        }),
                    ],
                }),
            }),
        ],
    });
}
