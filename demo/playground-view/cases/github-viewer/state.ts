// state.ts — the github-viewer's shared state + actions.
//
// Module-level ON PURPOSE: this state is shared across files (index.ts +
// landing.ts + explorer.ts consume the same atoms/controllers from several
// view factories). This is the "shared state" home the local-state idiom
// carves out — state used by a single view would live inside that view's
// function instead (view fns run exactly once, at build, so view-local
// atoms are stable).
import { createAnimationController } from "tur:animation";
import { filePicker } from "tur:filepicker";
import { request } from "tur:net";
import {
    createSvgResource,
    createTextEditingController,
    decodeUtf8,
    derive,
    isCancelError,
    mutate,
    type Readable,
    type StoreCtx,
    sleep,
    source,
    type Task,
    watch,
} from "tur:std";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface Repo {
    owner: string;
    repo: string;
    fullName: string; // "owner/repo"
}

export interface DirEntry {
    name: string; // immediate child name at the current path level
    path: string; // repo-relative path (no leading slash) — selection key
    isDir: boolean;
    size: number; // bytes for files; 0 for dirs
    downloadUrl: string | null; // cdn.jsdelivr.net raw URL (files only)
}

// ---------------------------------------------------------------------------
// Host bridge — HTTP via `tur:net`, file save via `tur:filepicker`.
// Both are registered by the embedder (tur-wasm wires the browser backends;
// tur-android wires NoopFilePicker). `hasHttp` is true whenever the case loads.
// ---------------------------------------------------------------------------

interface HttpResponse {
    ok: boolean;
    status: number;
    statusText: string;
    headers: Record<string, string>;
    body: ArrayBuffer;
}

interface HttpRequestOpts {
    url: string;
    method?: string;
    headers?: Record<string, string>;
    body?: string | ArrayBuffer;
}

export const hasHttp = typeof request === "function";

function http(opts: HttpRequestOpts): Promise<HttpResponse> {
    return request(opts).promise as Promise<HttpResponse>;
}

// ---------------------------------------------------------------------------
// State atoms
// ---------------------------------------------------------------------------

export const repo$ = source<Repo | null>(null);
export const version$ = source<string | null>(null); // resolved version tag
const fileTree$ = source<GhFile[] | null>(null); // flat tree from jsDelivr
export const view$ = source<"landing" | "explorer">("landing");
export const pathSegments$ = source<string[]>([]);
export const loading$ = source(false);
export const error$ = source<string | null>(null);
export const selectedPath$ = source<string | null>(null);

// Landing-screen draft + inline validation.
export const repoDraft$ = source("");
export const repoError$ = source<string | null>(null);

// Download feedback — decoupled from `loading$` (which is tree-load only).
// The previous version reused `loading$`, but the list view only surfaces
// "Loading…" when the list is empty, so an in-flight download showed nothing.
export type DownloadStatus = "idle" | "loading" | "done" | "error";
export const downloadStatus$ = source<DownloadStatus>("idle");

// Indeterminate spinner — an infinite animation controller that writes its
// eased progress (0..1) into `spinProgress$`. Stopped by default; `forward()`
// on download start, `stop()` on completion. We can't show real byte progress
// because `request` resolves the entire body in one shot.
export const spinProgress$ = source(0);
const spinCtrl = createAnimationController({
    duration: 900,
    curve: "linear",
    repeat: "infinite",
    onTick: mutate((ctx: StoreCtx, v: number) => ctx.set(spinProgress$, v)),
});

// ---------------------------------------------------------------------------
// Derived
// ---------------------------------------------------------------------------

/** Immediate children of the current path — computed locally from the flat
 *  tree + `pathSegments$`. No HTTP. Folder clicks / breadcrumbs just mutate
 *  `pathSegments$` and this recomputes instantly. */
export const entries$: Readable<DirEntry[]> = derive((ctx) => {
    const tree = ctx.get(fileTree$);
    if (!tree) return [];
    const segments = ctx.get(pathSegments$);
    const repo = ctx.get(repo$);
    const ver = ctx.get(version$);
    const prefix = segments.length === 0 ? "" : `${segments.join("/")}/`;
    const seen = new Map<string, DirEntry>();
    for (const f of tree) {
        // jsDelivr paths look like "/src/foo.js" — strip the leading slash.
        const p = f.name.startsWith("/") ? f.name.slice(1) : f.name;
        if (prefix.length === 0) {
            if (!p.includes("/")) {
                seen.set(p, {
                    name: p,
                    path: p,
                    isDir: false,
                    size: f.size ?? 0,
                    downloadUrl: rawUrl(repo, ver, p),
                });
            } else {
                const dir = p.slice(0, p.indexOf("/"));
                if (!seen.has(dir))
                    seen.set(dir, {
                        name: dir,
                        path: dir,
                        isDir: true,
                        size: 0,
                        downloadUrl: null,
                    });
            }
        } else {
            if (!p.startsWith(prefix)) continue;
            const rest = p.slice(prefix.length);
            if (!rest) continue;
            if (!rest.includes("/")) {
                seen.set(p, {
                    name: rest,
                    path: p,
                    isDir: false,
                    size: f.size ?? 0,
                    downloadUrl: rawUrl(repo, ver, p),
                });
            } else {
                const dir = rest.slice(0, rest.indexOf("/"));
                const fullPath = `${prefix}${dir}`;
                if (!seen.has(dir))
                    seen.set(dir, {
                        name: dir,
                        path: fullPath,
                        isDir: true,
                        size: 0,
                        downloadUrl: null,
                    });
            }
        }
    }
    const list = Array.from(seen.values());
    list.sort((a, b) =>
        a.isDir === b.isDir ? a.name.localeCompare(b.name) : a.isDir ? -1 : 1,
    );
    return list;
});

export const selectedEntry$: Readable<DirEntry | null> = derive((ctx) => {
    const path = ctx.get(selectedPath$);
    if (!path) return null;
    return ctx.get(entries$).find((e) => e.path === path) ?? null;
});

// ---------------------------------------------------------------------------
// Text controllers
// ---------------------------------------------------------------------------

/** Controller bound to `repoDraft$`; Enter submits the landing form. */
export const repoCtrl = createTextEditingController({
    onInput: mutate((ctx: StoreCtx, text: string, enter: boolean) => {
        ctx.set(repoDraft$, text);
        if (enter) ctx.set(openRepoFromDraft);
    }),
});

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

interface GhFile {
    name: string; // "/path/to/file"
    size?: number;
    hash?: string;
}

function errMsg(e: unknown): string {
    if (e && typeof e === "object" && "message" in e) {
        return String((e as { message: unknown }).message);
    }
    return String(e);
}

function fmtSize(bytes: number): string {
    if (!bytes) return "—";
    const units = ["B", "KB", "MB", "GB"];
    let i = 0;
    let n = bytes;
    while (n >= 1024 && i < units.length - 1) {
        n /= 1024;
        i++;
    }
    return `${n.toFixed(i === 0 ? 0 : 1)} ${units[i]}`;
}

export { fmtSize };

/** Parse a "owner/repo" string. Returns null if the shape is invalid. */
export function parseRepo(input: string): Repo | null {
    const parts = input
        .trim()
        .split("/")
        .map((p) => p.trim())
        .filter(Boolean);
    if (parts.length !== 2) return null;
    const [owner, repo] = parts;
    const repoClean = repo.replace(/\.git$/, "");
    if (!owner || !repoClean) return null;
    return { owner, repo: repoClean, fullName: `${owner}/${repoClean}` };
}

/** Build a cdn.jsdelivr.net raw URL for a file path. Null until version is known. */
function rawUrl(
    repo: Repo | null,
    version: string | null,
    path: string,
): string | null {
    if (!repo || !version) return null;
    const segs = path.split("/").map(encodeURIComponent).join("/");
    return `https://cdn.jsdelivr.net/gh/${encodeURIComponent(repo.owner)}/${encodeURIComponent(repo.repo)}@${encodeURIComponent(version)}/${segs}`;
}

// ---------------------------------------------------------------------------
// jsDelivr backend — version resolve + flat-tree fetch.
//   • https://data.jsdelivr.com/v1/packages/gh/{owner}/{repo}            → versions
//   • https://data.jsdelivr.com/v1/packages/gh/{owner}/{repo}@{v}?structure=flat
//                                                                        → full file tree
//   • https://cdn.jsdelivr.net/gh/{owner}/{repo}@{v}/{path}             → raw file
// No per-IP rate limit (CDN-cached, production-grade). One repo open = 2 calls;
// all subsequent navigation is client-side (derived `entries$`).
// ---------------------------------------------------------------------------

interface JsDelivrVersion {
    versions: Array<{ version: string }>;
    /** Tag → version map. An EMPTY OBJECT when the repo has no tags — never
     *  an array (a previous revision assumed `Array<{version}>` and called
     *  `.find` on it, which threw "not a callable function" for tag-less
     *  repos like octocat/Hello-World). */
    tags?: Record<string, string>;
}

interface JsDelivrTree {
    files?: GhFile[];
    status?: number;
    message?: string;
}

function pickVersion(body: JsDelivrVersion): string | null {
    // Prefer the latest published version; fall back to a `main`/`master`/
    // `HEAD` tag.
    if (body.versions && body.versions.length > 0)
        return body.versions[0].version;
    const tags = body.tags ?? {};
    return tags.main ?? tags.master ?? tags.HEAD ?? null;
}

/** Fetch the flat file tree for a repo, stash it, and switch to the explorer.
 *  Runs as two sequential HTTP calls (version resolve → tree). Dispatched by
 *  the `target$` watcher on every target change; the async fetch captures
 *  the mutation ctx. */
const loadRepo = mutate((ctx: StoreCtx, target: Repo) => {
    ctx.set(loading$, true);
    ctx.set(error$, null);
    ctx.set(fileTree$, null);
    ctx.set(selectedPath$, null);
    ctx.set(version$, null);

    const base = `https://data.jsdelivr.com/v1/packages/gh/${encodeURIComponent(target.owner)}/${encodeURIComponent(target.repo)}`;

    (async () => {
        try {
            const rVer = await http({
                method: "GET",
                url: base,
            });
            if (rVer.status === 404) {
                ctx.set(error$, `Repository not found: ${target.fullName}`);
                return;
            }
            if (rVer.status !== 200) {
                ctx.set(
                    error$,
                    `HTTP ${rVer.status} ${rVer.statusText}`.trim(),
                );
                return;
            }
            let parsed: JsDelivrVersion;
            try {
                parsed = JSON.parse(decodeUtf8(rVer.body));
            } catch {
                ctx.set(error$, "Bad response from jsDelivr");
                return;
            }
            const ver = pickVersion(parsed);
            if (!ver) {
                ctx.set(error$, `No published versions for ${target.fullName}`);
                return;
            }
            ctx.set(version$, ver);
            const rTree = await http({
                method: "GET",
                url: `${base}@${encodeURIComponent(ver)}?structure=flat`,
            });
            if (rTree.status !== 200) {
                // jsDelivr returns 403 for repos exceeding the 50 MB cap.
                let msg = `HTTP ${rTree.status} ${rTree.statusText}`.trim();
                try {
                    const body: JsDelivrTree = JSON.parse(
                        decodeUtf8(rTree.body),
                    );
                    if (
                        rTree.status === 403 &&
                        (body.message ?? "").includes("size")
                    ) {
                        msg = `${target.fullName} is too large for the CDN-backed viewer (50 MB limit). Try a smaller repo.`;
                    } else if (body.message) {
                        msg = body.message;
                    }
                } catch {
                    /* keep HTTP status string */
                }
                ctx.set(error$, msg);
                return;
            }
            let parsedTree: JsDelivrTree;
            try {
                parsedTree = JSON.parse(decodeUtf8(rTree.body));
            } catch {
                ctx.set(error$, "Bad file-tree response from jsDelivr");
                return;
            }
            ctx.set(fileTree$, parsedTree.files ?? []);
        } catch (e) {
            ctx.set(error$, errMsg(e));
        } finally {
            ctx.set(loading$, false);
        }
    })();
});

// ---------------------------------------------------------------------------
// Load flow — a `watch` on `target$`. Writing a fresh target object (any
// change of `repo` or `nonce`) re-runs the fetch; the nonce forces a
// refetch of the same repo because writes compare object values by
// reference (`refresh` bumps it). Started/stopped by the GithubViewer
// component's own lifecycleView (index.ts) so the watcher lives exactly as
// long as the component's subtree. The callback (`onTargetChange`) only dispatches `loadRepo`
// — it never writes `target$`, satisfying the watch-loop rule.
// ---------------------------------------------------------------------------

interface LoadTarget {
    repo: Repo;
    nonce: number;
}

export const target$ = source<LoadTarget | null>(null);

/** The watcher callback: dispatches `loadRepo` for the current target. A
 *  mutation like any other (`onMounted$`-style convention — `watch` takes
 *  the handle, not a raw closure). */
const onTargetChange = mutate((ctx: StoreCtx) => {
    const target = ctx.get(target$);
    if (target) ctx.set(loadRepo, target.repo);
});

export const repoWatch = watch(target$, onTargetChange);

// ---------------------------------------------------------------------------
// Navigation — all local (mutates `pathSegments$`; `entries$` recomputes).
// ---------------------------------------------------------------------------

export const openRepo = mutate((ctx: StoreCtx, repo: Repo) => {
    ctx.set(repo$, repo);
    ctx.set(pathSegments$, []);
    ctx.set(view$, "explorer");
    ctx.set(error$, null);
    ctx.set(target$, { repo, nonce: 0 });
});

/** Landing-screen submit: parse the draft and open the repo, or surface
 *  an inline format error. */
export const openRepoFromDraft = mutate((ctx: StoreCtx) => {
    const draft = ctx.get(repoDraft$);
    const repo = parseRepo(draft);
    if (!repo) {
        ctx.set(
            repoError$,
            'Enter a repo as "owner/name" — e.g. facebook/react',
        );
        return;
    }
    ctx.set(repoError$, null);
    ctx.set(openRepo, repo);
});

export const backToLanding = mutate((ctx: StoreCtx) => {
    spinCtrl.stop();
    statusTask?.cancel();
    statusTask = null;
    ctx.set(view$, "landing");
    ctx.set(repo$, null);
    ctx.set(version$, null);
    ctx.set(fileTree$, null);
    ctx.set(pathSegments$, []);
    ctx.set(selectedPath$, null);
    ctx.set(error$, null);
    ctx.set(downloadStatus$, "idle");
});

/** Navigate into a sub-folder (a row click on a directory). Local. */
export const openFolder = mutate((ctx: StoreCtx, entry: DirEntry) => {
    if (!entry.isDir) return;
    ctx.set(pathSegments$, [...ctx.get(pathSegments$), entry.name]);
    ctx.set(selectedPath$, null);
});

/** Up one folder level; if already at the repo root, return to landing. */
export const navigateUp = mutate((ctx: StoreCtx) => {
    const segs = ctx.get(pathSegments$);
    if (segs.length === 0) {
        ctx.set(backToLanding);
        return;
    }
    ctx.set(pathSegments$, segs.slice(0, -1));
    ctx.set(selectedPath$, null);
});

/** Navigate to the repo root (the breadcrumb's repo-name crumb). */
export const navigateToRoot = mutate((ctx: StoreCtx) => {
    ctx.set(pathSegments$, []);
    ctx.set(selectedPath$, null);
});

/** Re-fetch the tree (the only nav-adjacent HTTP call). Bumps the target's
 *  nonce so even the same repo produces a fresh (reference-unequal) target
 *  object — the write would otherwise be equality-gated away. */
export const refresh = mutate((ctx: StoreCtx) => {
    const repo = ctx.get(repo$);
    if (!repo) return;
    const prev = ctx.get(target$);
    ctx.set(target$, { repo, nonce: (prev?.nonce ?? 0) + 1 });
});

export const selectEntry = mutate((ctx: StoreCtx, entry: DirEntry) => {
    ctx.set(selectedPath$, entry.path);
});

// ---------------------------------------------------------------------------
// Download — fetch the file's raw bytes via cdn.jsdelivr.net and hand them
// to the host file-saver. Drives `downloadStatus$` for button feedback.
// ---------------------------------------------------------------------------

/** Revert the button to idle after a short confirmation window so the user
 *  sees the "Saved" / "Failed" flash before the label resets. */
const STATUS_FLASH_MS = 1800;
let statusTask: Task<void> | null = null;
const flashStatus = mutate((ctx: StoreCtx, status: DownloadStatus) => {
    statusTask?.cancel();
    ctx.set(downloadStatus$, status);
    if (status === "error") {
        // Keep error$ set — the banner will show it.
    }
    if (status !== "loading") {
        statusTask = sleep(STATUS_FLASH_MS);
        statusTask.promise.then(
            () => {
                statusTask = null;
                ctx.set(downloadStatus$, "idle");
            },
            () => {},
        );
    }
});

export const doDownload = mutate((ctx: StoreCtx) => {
    // Guard re-entry: ignore clicks while a download or its flash is active.
    if (ctx.get(downloadStatus$) !== "idle") return;
    const save = filePicker.saveFile;
    const entry = ctx.get(selectedEntry$);
    if (!entry || entry.isDir || !entry.downloadUrl) return;
    // Capture the narrowed string before the closure — TS does not preserve
    // property narrowing across async/callback boundaries.
    const downloadUrl = entry.downloadUrl;
    ctx.set(error$, null);
    ctx.set(flashStatus, "loading");
    spinCtrl.forward();
    (async () => {
        try {
            const r = await http({
                method: "GET",
                url: downloadUrl,
            });
            spinCtrl.stop();
            if (r.status >= 200 && r.status < 300 && r.body.byteLength > 0) {
                // Flip status to "done" first so the button morph renders on
                // the next frame. Defer `save()` by 500 ms (engine time) so the
                // "Saved" confirmation is already on screen before the host's
                // `<a>.click()` fires — that download side-effect can briefly
                // stall the frame loop, and without the defer the stall would
                // prevent the done-state flush from rendering.
                ctx.set(flashStatus, "done");
                const bytes = r.body;
                const name = entry.name;
                await sleep(500).promise;
                save(name, bytes);
            } else {
                ctx.set(error$, `Download failed: HTTP ${r.status}`);
                ctx.set(flashStatus, "error");
            }
        } catch (e) {
            if (isCancelError(e)) return;
            spinCtrl.stop();
            ctx.set(error$, errMsg(e));
            ctx.set(flashStatus, "error");
        }
    })();
});

// ---------------------------------------------------------------------------
// SVG icons — rasterised up front via createSvgResource.
// ---------------------------------------------------------------------------

const ICON_SVGS: Record<string, string> = {
    folder: `<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="#6366f1"><path d="M3 7a2 2 0 0 1 2-2h3.5a2 2 0 0 1 1.5.7l1 1.3H19a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/></svg>`,
    folderSoft: `<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="#a5b4fc"><path d="M3 7a2 2 0 0 1 2-2h3.5a2 2 0 0 1 1.5.7l1 1.3H19a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/></svg>`,
    file: `<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="#64748b" stroke-width="2" stroke-linejoin="round" stroke-linecap="round"><path d="M14 3v5h5"/><path d="M14 3H6a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/></svg>`,
    back: `<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="#475569" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round"><polyline points="15 18 9 12 15 6"/></svg>`,
    refresh: `<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="#475569" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><polyline points="21 4 21 10 15 10"/><path d="M3.5 12a8 8 0 0 1 14-4l3.5 2"/><polyline points="3 20 3 14 9 14"/><path d="M20.5 12a8 8 0 0 1-14 4L3 14"/></svg>`,
    download: `<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="#475569" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>`,
    spinner: `<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="#4f46e5" stroke-width="2.6" stroke-linecap="round"><path d="M21 12a9 9 0 1 1-6.219-8.56"/></svg>`,
    check: `<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="#ffffff" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"><polyline points="20 6 9 17 4 12"/></svg>`,
    github: `<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="#4f46e5"><path d="M12 .5C5.7.5.5 5.7.5 12c0 5.1 3.3 9.4 7.9 10.9.6.1.8-.2.8-.5v-2c-3.2.7-3.9-1.4-3.9-1.4-.5-1.3-1.3-1.7-1.3-1.7-1.1-.7.1-.7.1-.7 1.2.1 1.8 1.2 1.8 1.2 1 1.8 2.7 1.3 3.4 1 .1-.8.4-1.3.7-1.6-2.6-.3-5.3-1.3-5.3-5.7 0-1.3.4-2.3 1.2-3.1-.1-.3-.5-1.5.1-3.1 0 0 1-.3 3.2 1.2a11 11 0 0 1 5.8 0c2.2-1.5 3.2-1.2 3.2-1.2.6 1.6.2 2.8.1 3.1.8.8 1.2 1.8 1.2 3.1 0 4.4-2.7 5.4-5.3 5.7.4.4.8 1.1.8 2.2v3.3c0 .3.2.6.8.5 4.6-1.5 7.9-5.8 7.9-10.9C23.5 5.7 18.3.5 12 .5z"/></svg>`,
};

const iconIds: Record<string, number> = {};
for (const name of Object.keys(ICON_SVGS)) {
    iconIds[name] = createSvgResource(ICON_SVGS[name]);
}

export function getIcon(name: keyof typeof ICON_SVGS): number {
    return iconIds[name];
}
