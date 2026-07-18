import {
    Alignment,
    Axis,
    type Brush,
    Column,
    Condition,
    Container,
    CrossAxisAlignment,
    derive,
    Each,
    type Element,
    Expanded,
    get,
    Image,
    MainAxisSize,
    MouseRegion,
    mutate,
    PointerInteract,
    Row,
    ScrollView,
    SizedBox,
    type StoreCtx,
    Switch,
    set,
    source,
    Text,
} from "builtin:tur/std";
import { Transform } from "builtin:tur/animation";
import {
    type DirEntry,
    doDownload,
    downloadStatus$,
    entries$,
    error$,
    fmtSize,
    getIcon,
    loading$,
    navigateToRoot,
    navigateUp,
    openFolder,
    pathSegments$,
    refresh,
    repo$,
    selectEntry,
    selectedEntry$,
    selectedPath$,
    spinProgress$,
} from "./state";
import { COLORS } from "./theme";
import { IconButton } from "./ui";

// Per-row hover state (single source, not per-instance — keeps the
// subscription graph flat).
const hoveredPath$ = source<string | null>(null);

// --- File row -------------------------------------------------------------

function FileRow({
    entry,
    index,
}: {
    entry: DirEntry;
    index: number;
}): Element {
    return Column({
        crossAlignment: CrossAxisAlignment.Stretch,
        mainAxisSize: MainAxisSize.Min,
        children: [
            index === 0
                ? SizedBox({ width: 0, height: 0 })
                : SizedBox({ height: 4 }),
            MouseRegion({
                cursor: "pointer",
                onEnter: mutate((_ctx: StoreCtx, _ev) => {
                    set(hoveredPath$, entry.path);
                }),
                onExit: mutate((_ctx: StoreCtx, _ev) => {
                    set(hoveredPath$, null);
                }),
                child: PointerInteract({
                    onClick: mutate((_ctx: StoreCtx, _ev) => {
                        if (entry.isDir) openFolder(entry);
                        else selectEntry(entry);
                    }),
                    child: Container({
                        padding: 9,
                        borderRadius: 8,
                        color: derive(() => {
                            const sel = get(selectedPath$);
                            const hov = get(hoveredPath$);
                            if (sel === entry.path) return COLORS.rowSelected;
                            if (hov === entry.path) return COLORS.rowHover;
                            return COLORS.panel;
                        }),
                        children: [
                            Row({
                                children: [
                                    Image({
                                        resourceId: entry.isDir
                                            ? getIcon("folder")
                                            : getIcon("file"),
                                        width: 17,
                                        height: 17,
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
                                        color: COLORS.textSubtle,
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

function RepoCrumb(): Element {
    return MouseRegion({
        cursor: "pointer",
        child: PointerInteract({
            onClick: mutate((_ctx: StoreCtx, _ev) => navigateToRoot()),
            child: Container({
                padding: 4,
                children: [
                    Text({
                        text: derive(() => get(repo$)?.fullName ?? ""),
                        fontSize: 13,
                        color: COLORS.accent,
                    }),
                ],
            }),
        }),
    });
}

// --- Download button (reactive: idle → loading w/ spinner → done/error) --
// The button's whole body is swapped via a `Switch` on `downloadStatus$` so a
// status change structurally mounts a new subtree (spinner / check / label)
// rather than relying on prop-level re-resolution.

function Spinner(): Element {
    return Transform({
        rotate: derive(() => get(spinProgress$) * 2 * Math.PI),
        child: Image({
            resourceId: getIcon("spinner"),
            width: 14,
            height: 14,
            queryKey: ["dl-spinner"],
        }),
    });
}

function CheckIcon(): Element {
    return Image({
        resourceId: getIcon("check"),
        width: 14,
        height: 14,
        queryKey: ["dl-check"],
    });
}

/** One button body: coloured pill with an optional leading icon + label. */
function dlShell(
    bg: unknown,
    fg: unknown,
    label: string,
    leading: Element | null,
): Element {
    const textEl = Text({ text: label, fontSize: 13, color: fg as Brush });
    return Container({
        padding: 7,
        borderRadius: 7,
        color: bg as Brush,
        children: [
            Row({
                mainAxisSize: MainAxisSize.Min,
                children: leading
                    ? [leading, SizedBox({ width: 6 }), textEl]
                    : [textEl],
            }),
        ],
    });
}

function DownloadButton(): Element {
    return MouseRegion({
        cursor: "pointer",
        child: PointerInteract({
            onClick: mutate((_ctx: StoreCtx, _ev) => {
                if (get(downloadStatus$) !== "idle") return;
                const e = get(selectedEntry$);
                if (e && !e.isDir) doDownload();
            }),
            child: Switch({
                value: derive(() => get(downloadStatus$)),
                cases: [
                    {
                        key: "loading",
                        child: () =>
                            dlShell(
                                COLORS.accentSoft,
                                COLORS.accent,
                                "Downloading…",
                                Spinner(),
                            ),
                    },
                    {
                        key: "done",
                        child: () =>
                            dlShell(
                                COLORS.success,
                                COLORS.accentFg,
                                "Saved",
                                CheckIcon(),
                            ),
                    },
                    {
                        key: "error",
                        child: () =>
                            dlShell(
                                COLORS.dangerSoft,
                                COLORS.danger,
                                "Failed",
                                null,
                            ),
                    },
                ],
                fallback: () =>
                    dlShell(
                        derive(() => {
                            const e = get(selectedEntry$);
                            return e && !e.isDir
                                ? COLORS.accent
                                : COLORS.subtleButton;
                        }),
                        derive(() => {
                            const e = get(selectedEntry$);
                            return e && !e.isDir
                                ? COLORS.accentFg
                                : COLORS.textSubtle;
                        }),
                        "Download",
                        null,
                    ),
            }),
        }),
    });
}

// --- Explorer screen ------------------------------------------------------

export function ExplorerScreen(): Element {
    // Top bar + toolbar are bare `Row`s (content-sized) — wrapping them in a
    // `Container` would inflate to the Column's full height and starve the
    // `Expanded` file list.
    return Column({
        crossAlignment: CrossAxisAlignment.Stretch,
        children: [
            // Top bar: back (up one level / back to landing) + repo crumb + path.
            Row({
                crossAlignment: CrossAxisAlignment.Center,
                children: [
                    IconButton({
                        resourceId: getIcon("back"),
                        onClick: (_ctx) => navigateUp(),
                    }),
                    SizedBox({ width: 8 }),
                    RepoCrumb(),
                    // Path segments as a single reactive Text (avoids `Each`
                    // inflating this content-sized Row).
                    Text({
                        text: derive(() => {
                            const segs = get(pathSegments$);
                            return segs.length ? ` / ${segs.join(" / ")}` : "";
                        }),
                        fontSize: 13,
                        color: COLORS.textSubtle,
                    }),
                    Expanded({ child: SizedBox({ width: 0, height: 0 }) }),
                ],
            }),
            SizedBox({ height: 8 }),
            // Toolbar.
            Row({
                crossAlignment: CrossAxisAlignment.Center,
                children: [
                    IconButton({
                        resourceId: getIcon("refresh"),
                        onClick: refresh,
                    }),
                    SizedBox({ width: 8 }),
                    DownloadButton(),
                ],
            }),
            SizedBox({ height: 8 }),
            Condition({
                condition: derive(() => get(error$) !== null),
                child: () =>
                    Container({
                        padding: 10,
                        borderRadius: 8,
                        color: COLORS.dangerSoft,
                        children: [
                            Text({
                                text: derive(() => get(error$) ?? ""),
                                fontSize: 12,
                                color: COLORS.danger,
                            }),
                        ],
                    }),
                elseChild: () => SizedBox({ width: 0, height: 0 }),
            }),
            SizedBox({ height: 8 }),
            // File list (loading / empty / list). When an error is present
            // the banner above already explains it; render a blank area here
            // instead of the "folder is empty" state (which would mislead).
            Expanded({
                child: fileListView(),
            }),
        ],
    });
}

function fileListView(): Element {
    return Container({
        padding: 4,
        children: [
            Switch({
                value: derive(() => {
                    if (get(loading$) && get(entries$).length === 0)
                        return "loading";
                    if (get(entries$).length === 0)
                        return get(error$) !== null ? "blank" : "empty";
                    return "list";
                }),
                cases: [
                    {
                        key: "loading",
                        child: () =>
                            Container({
                                alignment: Alignment.Center,
                                children: [
                                    Text({
                                        text: "Loading…",
                                        fontSize: 13,
                                        color: COLORS.textSubtle,
                                    }),
                                ],
                            }),
                    },
                    {
                        key: "empty",
                        child: () => emptyFolder(),
                    },
                    {
                        key: "blank",
                        child: () => SizedBox({ width: 0, height: 0 }),
                    },
                    {
                        key: "list",
                        child: () =>
                            ScrollView({
                                axis: Axis.Vertical,
                                child: Column({
                                    crossAlignment: CrossAxisAlignment.Stretch,
                                    mainAxisSize: MainAxisSize.Min,
                                    children: [
                                        Each({
                                            items: entries$,
                                            crossAlignment:
                                                CrossAxisAlignment.Stretch,
                                            build: (e: DirEntry, i: number) =>
                                                FileRow({ entry: e, index: i }),
                                        }),
                                    ],
                                }),
                            }),
                    },
                ],
            }),
        ],
    });
}

function emptyFolder(): Element {
    return Container({
        alignment: Alignment.Center,
        padding: 40,
        children: [
            Column({
                crossAlignment: CrossAxisAlignment.Center,
                mainAxisSize: MainAxisSize.Min,
                children: [
                    Image({
                        resourceId: getIcon("folderSoft"),
                        width: 40,
                        height: 40,
                        queryKey: ["empty-icon"],
                    }),
                    SizedBox({ height: 12 }),
                    Text({
                        text: "This folder is empty",
                        fontSize: 14,
                        color: COLORS.text,
                    }),
                    SizedBox({ height: 6 }),
                    Text({
                        text: "No files to show at this path.",
                        fontSize: 12,
                        color: COLORS.textSubtle,
                    }),
                ],
            }),
        ],
    });
}
