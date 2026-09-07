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
    source,
    Text,
    Transform,
} from "tur:std";
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
    return Column()
        .crossAlignment(CrossAxisAlignment.Stretch)
        .mainAxisSize(MainAxisSize.Min)
        .children([
            index === 0
                ? SizedBox().width(0).height(0).build()
                : SizedBox().height(4).build(),
            MouseRegion()
                .cursor("pointer")
                .onEnter(
                    mutate((ctx: StoreCtx, _ev) => {
                        ctx.set(hoveredPath$, entry.path);
                    }),
                )
                .onExit(
                    mutate((ctx: StoreCtx, _ev) => {
                        ctx.set(hoveredPath$, null);
                    }),
                )
                .child(
                    PointerInteract()
                        .onClick(
                            mutate((ctx: StoreCtx, _ev) => {
                                if (entry.isDir) ctx.set(openFolder, entry);
                                else ctx.set(selectEntry, entry);
                            }),
                        )
                        .child(
                            Container()
                                .padding(9)
                                .borderRadius(8)
                                .color(
                                    derive((ctx) => {
                                        const sel = ctx.get(selectedPath$);
                                        const hov = ctx.get(hoveredPath$);
                                        if (sel === entry.path)
                                            return COLORS.rowSelected;
                                        if (hov === entry.path)
                                            return COLORS.rowHover;
                                        return COLORS.panel;
                                    }),
                                )
                                .children([
                                    Row()
                                        .children([
                                            Image({
                                                resourceId: entry.isDir
                                                    ? getIcon("folder")
                                                    : getIcon("file"),
                                            })
                                                .width(17)
                                                .height(17)
                                                .queryKey(["row-icon"])
                                                .build(),
                                            SizedBox().width(10).build(),
                                            Expanded()
                                                .child(
                                                    Text({ text: entry.name })
                                                        .fontSize(13)
                                                        .color(COLORS.text)
                                                        .build(),
                                                )
                                                .build(),
                                            SizedBox().width(10).build(),
                                            Text({
                                                text: entry.isDir
                                                    ? "Folder"
                                                    : fmtSize(entry.size),
                                            })
                                                .fontSize(11)
                                                .color(COLORS.textSubtle)
                                                .build(),
                                        ])
                                        .build(),
                                ])
                                .build(),
                        )
                        .build(),
                )
                .build(),
        ])
        .build();
}

// --- Breadcrumb -----------------------------------------------------------

function RepoCrumb(): Element {
    return MouseRegion()
        .cursor("pointer")
        .child(
            PointerInteract()
                .onClick(
                    mutate((ctx: StoreCtx, _ev) => ctx.set(navigateToRoot)),
                )
                .child(
                    Container()
                        .padding(4)
                        .children([
                            Text({
                                text: derive(
                                    (ctx) => ctx.get(repo$)?.fullName ?? "",
                                ),
                            })
                                .fontSize(13)
                                .color(COLORS.accent)
                                .build(),
                        ])
                        .build(),
                )
                .build(),
        )
        .build();
}

// --- Download button (reactive: idle → loading w/ spinner → done/error) --
// The button's whole body is swapped via a `Switch` on `downloadStatus$` so a
// status change structurally mounts a new subtree (spinner / check / label)
// rather than relying on prop-level re-resolution.

function Spinner(): Element {
    return Transform()
        .rotate(derive((ctx) => ctx.get(spinProgress$) * 2 * Math.PI))
        .child(
            Image({ resourceId: getIcon("spinner") })
                .width(14)
                .height(14)
                .queryKey(["dl-spinner"])
                .build(),
        )
        .build();
}

function CheckIcon(): Element {
    return Image({ resourceId: getIcon("check") })
        .width(14)
        .height(14)
        .queryKey(["dl-check"])
        .build();
}

/** One button body: coloured pill with an optional leading icon + label. */
function dlShell(
    bg: unknown,
    fg: unknown,
    label: string,
    leading: Element | null,
): Element {
    const textEl = Text({ text: label })
        .fontSize(13)
        .color(fg as Brush)
        .build();
    return Container()
        .padding(7)
        .borderRadius(7)
        .color(bg as Brush)
        .children([
            Row()
                .mainAxisSize(MainAxisSize.Min)
                .children(
                    leading
                        ? [leading, SizedBox().width(6).build(), textEl]
                        : [textEl],
                )
                .build(),
        ])
        .build();
}

function DownloadButton(): Element {
    return MouseRegion()
        .cursor("pointer")
        .child(
            PointerInteract()
                .onClick(mutate((_ctx: StoreCtx, _ev) => _ctx.set(doDownload)))
                .child(
                    Switch({ value: derive((ctx) => ctx.get(downloadStatus$)) })
                        .cases([
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
                        ])
                        .fallback(() =>
                            dlShell(
                                derive((ctx) => {
                                    const e = ctx.get(selectedEntry$);
                                    return e && !e.isDir
                                        ? COLORS.accent
                                        : COLORS.subtleButton;
                                }),
                                derive((ctx) => {
                                    const e = ctx.get(selectedEntry$);
                                    return e && !e.isDir
                                        ? COLORS.accentFg
                                        : COLORS.textSubtle;
                                }),
                                "Download",
                                null,
                            ),
                        )
                        .build(),
                )
                .build(),
        )
        .build();
}

// --- Explorer screen ------------------------------------------------------

export function ExplorerScreen(): Element {
    // Top bar + toolbar are bare `Row`s (content-sized) — wrapping them in a
    // `Container` would inflate to the Column's full height and starve the
    // `Expanded` file list.
    return Column()
        .crossAlignment(CrossAxisAlignment.Stretch)
        .children([
            // Top bar: back (up one level / back to landing) + repo crumb + path.
            Row()
                .crossAlignment(CrossAxisAlignment.Center)
                .children([
                    IconButton({
                        resourceId: getIcon("back"),
                        onClick: navigateUp,
                    }),
                    SizedBox().width(8).build(),
                    RepoCrumb(),
                    // Path segments as a single reactive Text (avoids `Each`
                    // inflating this content-sized Row).
                    Text({
                        text: derive((ctx) => {
                            const segs = ctx.get(pathSegments$);
                            return segs.length ? ` / ${segs.join(" / ")}` : "";
                        }),
                    })
                        .fontSize(13)
                        .color(COLORS.textSubtle)
                        .build(),
                    Expanded()
                        .child(SizedBox().width(0).height(0).build())
                        .build(),
                ])
                .build(),
            SizedBox().height(8).build(),
            // Toolbar.
            Row()
                .crossAlignment(CrossAxisAlignment.Center)
                .children([
                    IconButton({
                        resourceId: getIcon("refresh"),
                        onClick: refresh,
                    }),
                    SizedBox().width(8).build(),
                    DownloadButton(),
                ])
                .build(),
            SizedBox().height(8).build(),
            Condition({ condition: derive((ctx) => ctx.get(error$) !== null) })
                .elseChild(() => SizedBox().width(0).height(0).build())
                .child(() =>
                    Container()
                        .padding(10)
                        .borderRadius(8)
                        .color(COLORS.dangerSoft)
                        .children([
                            Text({
                                text: derive((ctx) => ctx.get(error$) ?? ""),
                            })
                                .fontSize(12)
                                .color(COLORS.danger)
                                .build(),
                        ])
                        .build(),
                )
                .build(),
            SizedBox().height(8).build(),
            // File list (loading / empty / list). When an error is present
            // the banner above already explains it; render a blank area here
            // instead of the "folder is empty" state (which would mislead).
            Expanded().child(fileListView()).build(),
        ])
        .build();
}

function fileListView(): Element {
    return Container()
        .padding(4)
        .children([
            Switch({
                value: derive((ctx) => {
                    if (ctx.get(loading$) && ctx.get(entries$).length === 0)
                        return "loading";
                    if (ctx.get(entries$).length === 0)
                        return ctx.get(error$) !== null ? "blank" : "empty";
                    return "list";
                }),
            })
                .cases([
                    {
                        key: "loading",
                        child: () =>
                            Container()
                                .alignment(Alignment.Center)
                                .children([
                                    Text({ text: "Loading…" })
                                        .fontSize(13)
                                        .color(COLORS.textSubtle)
                                        .build(),
                                ])
                                .build(),
                    },
                    {
                        key: "empty",
                        child: () => emptyFolder(),
                    },
                    {
                        key: "blank",
                        child: () => SizedBox().width(0).height(0).build(),
                    },
                    {
                        key: "list",
                        child: () =>
                            ScrollView()
                                .axis(Axis.Vertical)
                                .child(
                                    Column()
                                        .crossAlignment(
                                            CrossAxisAlignment.Stretch,
                                        )
                                        .mainAxisSize(MainAxisSize.Min)
                                        .children([
                                            Each({
                                                items: entries$,
                                            })
                                                .itemBuilder(
                                                    (e: DirEntry, i: number) =>
                                                        FileRow({
                                                            entry: e,
                                                            index: i,
                                                        }),
                                                )
                                                .build(),
                                        ])
                                        .build(),
                                )
                                .build(),
                    },
                ])
                .build(),
        ])
        .build();
}

function emptyFolder(): Element {
    return Container()
        .alignment(Alignment.Center)
        .padding(40)
        .children([
            Column()
                .crossAlignment(CrossAxisAlignment.Center)
                .mainAxisSize(MainAxisSize.Min)
                .children([
                    Image({ resourceId: getIcon("folderSoft") })
                        .width(40)
                        .height(40)
                        .queryKey(["empty-icon"])
                        .build(),
                    SizedBox().height(12).build(),
                    Text({ text: "This folder is empty" })
                        .fontSize(14)
                        .color(COLORS.text)
                        .build(),
                    SizedBox().height(6).build(),
                    Text({ text: "No files to show at this path." })
                        .fontSize(12)
                        .color(COLORS.textSubtle)
                        .build(),
                ])
                .build(),
        ])
        .build();
}
