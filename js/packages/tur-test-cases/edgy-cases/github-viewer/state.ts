import { saveFile } from "builtin:tur/host";
import { request } from "builtin:tur/net";
import {
    createAnimationController,
    createSvgResource,
    createTextEditingController,
    derive,
    get,
    mutate,
    type Readable,
    type StoreCtx,
    set,
    source,
} from "builtin:tur/std";

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
// Host bridge — HTTP via `builtin:tur/net`, file save via `builtin:tur/host`.
// Both are registered by tur-wasm (playground). The case is playground-only,
// so `hasHttp` is true whenever the case loads.
// ---------------------------------------------------------------------------

interface HttpResponse {
    ok: boolean;
    status: number;
    statusText: string;
    headers: Record<string, string>;
    bodyText?: string;
    bodyBytes?: ArrayBuffer;
}

interface HttpRequestOpts {
    url: string;
    method?: string;
    headers?: Record<string, string>;
    body?: string | ArrayBuffer;
    responseType?: "text" | "bytes";
}

export const hasHttp = typeof request === "function";

function http(opts: HttpRequestOpts): Promise<HttpResponse> {
    return request(opts) as Promise<HttpResponse>;
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
    onTick: mutate((_ctx: StoreCtx, v: number) => set(spinProgress$, v)),
});

// ---------------------------------------------------------------------------
// Derived
// ---------------------------------------------------------------------------

/** Immediate children of the current path — computed locally from the flat
 *  tree + `pathSegments$`. No HTTP. Folder clicks / breadcrumbs just mutate
 *  `pathSegments$` and this recomputes instantly. */
export const entries$: Readable<DirEntry[]> = derive(() => {
    const tree = get(fileTree$);
    if (!tree) return [];
    const segments = get(pathSegments$);
    const repo = get(repo$);
    const ver = get(version$);
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

export const selectedEntry$: Readable<DirEntry | null> = derive(() => {
    const path = get(selectedPath$);
    if (!path) return null;
    return get(entries$).find((e) => e.path === path) ?? null;
});

// ---------------------------------------------------------------------------
// Text controllers
// ---------------------------------------------------------------------------

/** Controller bound to `repoDraft$`; Enter submits the landing form. */
export const repoCtrl = createTextEditingController({
    onInput: mutate((ctx: StoreCtx, text: string, enter: boolean) => {
        ctx.set(repoDraft$, text);
        if (enter) openRepoFromDraft(ctx);
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
    tags?: Array<{ version: string }>;
}

interface JsDelivrTree {
    files?: GhFile[];
    status?: number;
    message?: string;
}

function pickVersion(body: JsDelivrVersion): string | null {
    // Prefer the latest release tag; fall back to a `main`/`master`/`HEAD` ref.
    if (body.versions && body.versions.length > 0)
        return body.versions[0].version;
    const tag = body.tags?.find(
        (t) =>
            t.version === "main" ||
            t.version === "master" ||
            t.version === "HEAD",
    );
    return tag?.version ?? null;
}

/** Fetch the flat file tree for a repo, stash it, and switch to the explorer.
 *  Runs as two sequential HTTP calls (version resolve → tree). */
function loadRepo(target: Repo): void {
    set(loading$, true);
    set(error$, null);
    set(fileTree$, null);
    set(selectedPath$, null);
    set(version$, null);

    const base = `https://data.jsdelivr.com/v1/packages/gh/${encodeURIComponent(target.owner)}/${encodeURIComponent(target.repo)}`;

    http({ method: "GET", url: base, responseType: "text" })
        .then((rVer) => {
            if (rVer.status === 404) {
                set(error$, `Repository not found: ${target.fullName}`);
                return;
            }
            if (rVer.status !== 200) {
                set(error$, `HTTP ${rVer.status} ${rVer.statusText}`.trim());
                return;
            }
            let parsed: JsDelivrVersion;
            try {
                parsed = JSON.parse(rVer.bodyText ?? "{}");
            } catch {
                set(error$, "Bad response from jsDelivr");
                return;
            }
            const ver = pickVersion(parsed);
            if (!ver) {
                set(error$, `No published versions for ${target.fullName}`);
                return;
            }
            set(version$, ver);
            return http({
                method: "GET",
                url: `${base}@${encodeURIComponent(ver)}?structure=flat`,
                responseType: "text",
            }).then((rTree) => {
                if (rTree.status !== 200) {
                    // jsDelivr returns 403 for repos exceeding the 50 MB cap.
                    let msg = `HTTP ${rTree.status} ${rTree.statusText}`.trim();
                    try {
                        const body: JsDelivrTree = JSON.parse(
                            rTree.bodyText ?? "{}",
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
                    set(error$, msg);
                    return;
                }
                let parsedTree: JsDelivrTree;
                try {
                    parsedTree = JSON.parse(rTree.bodyText ?? "{}");
                } catch {
                    set(error$, "Bad file-tree response from jsDelivr");
                    return;
                }
                set(fileTree$, parsedTree.files ?? []);
            });
        })
        .catch((e) => set(error$, errMsg(e)))
        .finally(() => set(loading$, false));
}

// ---------------------------------------------------------------------------
// Navigation — all local (mutates `pathSegments$`; `entries$` recomputes).
// ---------------------------------------------------------------------------

export function openRepo(ctx: StoreCtx, repo: Repo): void {
    ctx.set(repo$, repo);
    ctx.set(pathSegments$, []);
    ctx.set(view$, "explorer");
    ctx.set(error$, null);
    loadRepo(repo);
}

/** Landing-screen submit: parse the draft and open the repo, or surface
 *  an inline format error. Runs inside a mutation context. */
export function openRepoFromDraft(ctx: StoreCtx): void {
    const draft = get(repoDraft$);
    const repo = parseRepo(draft);
    if (!repo) {
        ctx.set(
            repoError$,
            'Enter a repo as "owner/name" — e.g. facebook/react',
        );
        return;
    }
    ctx.set(repoError$, null);
    openRepo(ctx, repo);
}

export function backToLanding(): void {
    spinCtrl.stop();
    if (statusTimer) {
        clearTimeout(statusTimer);
        statusTimer = null;
    }
    set(view$, "landing");
    set(repo$, null);
    set(version$, null);
    set(fileTree$, null);
    set(pathSegments$, []);
    set(selectedPath$, null);
    set(error$, null);
    set(downloadStatus$, "idle");
}

/** Navigate into a sub-folder (a row click on a directory). Local. */
export function openFolder(entry: DirEntry): void {
    if (!entry.isDir) return;
    set(pathSegments$, [...get(pathSegments$), entry.name]);
    set(selectedPath$, null);
}

/** Up one folder level; if already at the repo root, return to landing. */
export function navigateUp(): void {
    const segs = get(pathSegments$);
    if (segs.length === 0) {
        backToLanding();
        return;
    }
    set(pathSegments$, segs.slice(0, -1));
    set(selectedPath$, null);
}

/** Navigate to the repo root (the breadcrumb's repo-name crumb). */
export function navigateToRoot(): void {
    set(pathSegments$, []);
    set(selectedPath$, null);
}

/** Re-fetch the tree (the only nav-adjacent HTTP call). */
export function refresh(): void {
    const repo = get(repo$);
    if (repo) loadRepo(repo);
}

export function selectEntry(entry: DirEntry): void {
    set(selectedPath$, entry.path);
}

// ---------------------------------------------------------------------------
// Download — fetch the file's raw bytes via cdn.jsdelivr.net and hand them
// to the host file-saver. Drives `downloadStatus$` for button feedback.
// ---------------------------------------------------------------------------

/** Revert the button to idle after a short confirmation window so the user
 *  sees the "Saved" / "Failed" flash before the label resets. */
const STATUS_FLASH_MS = 1800;
let statusTimer: ReturnType<typeof setTimeout> | null = null;
function flashStatus(status: DownloadStatus): void {
    if (statusTimer) clearTimeout(statusTimer);
    set(downloadStatus$, status);
    if (status === "error") {
        // Keep error$ set — the banner will show it.
    }
    if (status !== "loading") {
        statusTimer = setTimeout(() => {
            statusTimer = null;
            set(downloadStatus$, "idle");
        }, STATUS_FLASH_MS);
    }
}

export function doDownload(): void {
    // Guard re-entry: ignore clicks while a download or its flash is active.
    if (get(downloadStatus$) !== "idle") return;
    const save = saveFile;
    if (!save) {
        set(error$, "Save not available in this host");
        flashStatus("error");
        return;
    }
    const entry = get(selectedEntry$);
    if (!entry || entry.isDir || !entry.downloadUrl) return;
    set(error$, null);
    flashStatus("loading");
    spinCtrl.forward();
    http({
        method: "GET",
        url: entry.downloadUrl,
        responseType: "bytes",
    })
        .then((r) => {
            spinCtrl.stop();
            if (r.status >= 200 && r.status < 300 && r.bodyBytes) {
                // Flip status to "done" first so the button morph renders on
                // the next frame. Defer `save()` by 500 ms (engine time) so the
                // "Saved" confirmation is already on screen before the host's
                // `<a>.click()` fires — that download side-effect can briefly
                // stall the frame loop, and without the defer the stall would
                // prevent the done-state flush from rendering.
                flashStatus("done");
                const bytes = r.bodyBytes;
                const name = entry.name;
                setTimeout(() => save(name, bytes as ArrayBuffer), 500);
            } else {
                set(error$, `Download failed: HTTP ${r.status}`);
                flashStatus("error");
            }
        })
        .catch((e) => {
            spinCtrl.stop();
            set(error$, errMsg(e));
            flashStatus("error");
        });
}

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
