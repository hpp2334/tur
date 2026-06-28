import {
    type Atom,
    createSvgResource,
    createTextEditingController,
    derive,
    get,
    mutate,
    type Readable,
    type StoreCtx,
    set,
    source,
} from "@tur/edgy";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface WebDavServer {
    id: number;
    name: string;
    url: string;
    username: string;
    password: string;
}

export interface DirEntry {
    name: string;
    href: string;
    isDir: boolean;
    size: number;
    modified: string;
}

export type TestStatus = "idle" | "testing" | "ok" | "fail";

// ---------------------------------------------------------------------------
// Host bridge — `__tur.request` (HTTP, Promise) + `__turHost` (file IO).
// Lives in tur-wasm; absent under the native engine, so the UI guards on
// `hasHttp` and shows a "requires the browser playground" notice otherwise.
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
    username?: string;
    password?: string;
}

interface PickedFile {
    name: string;
    bytes: ArrayBuffer;
}

type TurRequest = (opts: HttpRequestOpts) => Promise<HttpResponse>;

interface TurHost {
    pickFile?: (cb: (r: PickedFile | null) => void) => void;
    saveFile?: (name: string, bytes: ArrayBuffer) => void;
}

const TUR = (globalThis as unknown as { __tur?: { request?: TurRequest } })
    .__tur;
const HOST = (globalThis as unknown as { __turHost?: TurHost }).__turHost;
const TUR_REQUEST = TUR?.request;

export const hasHttp = typeof TUR_REQUEST === "function";

function http(opts: HttpRequestOpts): Promise<HttpResponse> {
    if (!TUR_REQUEST)
        return Promise.reject({ message: "__tur.request not available" });
    return TUR_REQUEST(opts);
}

// ---------------------------------------------------------------------------
// State atoms
// ---------------------------------------------------------------------------

export const servers$ = source<WebDavServer[]>([]);
export const view$ = source<"list" | "explorer">("list");
export const currentServerId$ = source<number | null>(null);
export const pathSegments$ = source<string[]>([]);
export const entries$ = source<DirEntry[]>([]);
export const loading$ = source(false);
export const error$ = source<string | null>(null);
export const selectedHref$ = source<string | null>(null);

// Modal state
export const connectOpen$ = source(false);
export const editingServer$ = source<WebDavServer | null>(null);
export const newFolderOpen$ = source(false);
export const confirmDelete$ = source<DirEntry | null>(null);

// Connect-dialog test status
export const testStatus$ = source<TestStatus>("idle");
export const testMessage$ = source("");

// Draft atoms for the connect dialog fields
export const nameDraft$ = source("");
export const urlDraft$ = source("");
export const userDraft$ = source("");
export const passDraft$ = source("");
export const newFolderDraft$ = source("");

// ---------------------------------------------------------------------------
// Derived
// ---------------------------------------------------------------------------

export const currentServer$ = derive<WebDavServer | null>(() => {
    const id = get(currentServerId$);
    if (id === null) return null;
    return get(servers$).find((s) => s.id === id) ?? null;
});

export const selectedEntry$: Readable<DirEntry | null> = derive(() => {
    const href = get(selectedHref$);
    if (!href) return null;
    return get(entries$).find((e) => e.href === href) ?? null;
});

// ---------------------------------------------------------------------------
// Text controllers (draft-backed, mirroring the todolist pattern)
// ---------------------------------------------------------------------------

interface TextController {
    setSpans(spans: Array<{ content: string; color?: unknown }>): void;
    readonly text: string;
}

function draftCtrl(atom: Atom<string>): TextController {
    return createTextEditingController({
        onInput: mutate((ctx: StoreCtx, text: string, _enter: boolean) => {
            ctx.set(atom, text);
        }),
    }) as unknown as TextController;
}

export const nameCtrl = draftCtrl(nameDraft$);
export const urlCtrl = draftCtrl(urlDraft$);
export const userCtrl = draftCtrl(userDraft$);
export const passCtrl = draftCtrl(passDraft$);
export const newFolderCtrl = draftCtrl(newFolderDraft$);

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

let nextId = 1;

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

/** Join a server base URL with path segments into a collection URL ending in "/". */
function joinUrl(base: string, segments: string[]): string {
    const b = base.replace(/\/+$/, "");
    return `${b}${segments.length ? `/${segments.map(encodeURIComponent).join("/")}` : ""}/`;
}

const PROPFIND_BODY =
    '<?xml version="1.0" encoding="utf-8"?>\n' +
    '<D:propfind xmlns:D="DAV:"><D:prop>' +
    "<D:resourcetype/><D:getcontentlength/><D:getlastmodified/>" +
    "</D:prop></D:propfind>";

/** Lightweight WebDAV multistatus parser. Tolerant of namespace prefixes
 *  (d:, D:, dav:, none). Skips the queried collection (first response). */
function parsePropfind(xml: string): DirEntry[] {
    const out: DirEntry[] = [];
    const responseRe =
        /<(?:[a-zA-Z0-9]+:)?response\b[^>]*>([\s\S]*?)<\/(?:[a-zA-Z0-9]+:)?response>/gi;
    const pick = (block: string, name: string): string | null => {
        const re = new RegExp(
            `<(?:[a-zA-Z0-9]+:)?${name}\\b[^>]*>([\\s\\S]*?)<\\/(?:[a-zA-Z0-9]+:)?${name}>`,
            "i",
        );
        const m = re.exec(block);
        return m ? m[1].trim() : null;
    };
    let first = true;
    let m = responseRe.exec(xml);
    while (m !== null) {
        const block = m[1];
        if (first) {
            // The first <response> is conventionally the queried collection.
            first = false;
            m = responseRe.exec(xml);
            continue;
        }
        const hrefRaw = pick(block, "href");
        if (!hrefRaw) {
            m = responseRe.exec(xml);
            continue;
        }
        let href: string;
        try {
            href = decodeURIComponent(hrefRaw);
        } catch {
            href = hrefRaw;
        }
        const segs = href.split("/").filter(Boolean);
        const name = segs[segs.length - 1] ?? "";
        if (!name) {
            m = responseRe.exec(xml);
            continue;
        }
        const isDir =
            /<(?:[a-zA-Z0-9]+:)?collection\s*\/?>/i.test(block) ||
            href.endsWith("/");
        const sizeRaw = pick(block, "getcontentlength");
        const size = sizeRaw ? Number.parseInt(sizeRaw, 10) || 0 : 0;
        const modified = pick(block, "getlastmodified") ?? "";
        out.push({ name, href, isDir, size, modified });
        m = responseRe.exec(xml);
    }
    return out.sort((a, b) =>
        a.isDir === b.isDir ? a.name.localeCompare(b.name) : a.isDir ? -1 : 1,
    );
}

// ---------------------------------------------------------------------------
// WebDAV operations — plain functions using module-level get/set. The promise
// bodies run as PromiseJobs inside the engine's flush, so their set()s land in
// the same reactive pass.
// ---------------------------------------------------------------------------

export function doList(server: WebDavServer, segments: string[]): void {
    set(loading$, true);
    set(error$, null);
    set(selectedHref$, null);
    http({
        method: "PROPFIND",
        url: joinUrl(server.url, segments),
        username: server.username,
        password: server.password,
        headers: {
            Depth: "1",
            "Content-Type": "application/xml; charset=utf-8",
        },
        body: PROPFIND_BODY,
        responseType: "text",
    })
        .then((r) => {
            if (r.status === 207 || r.status === 200) {
                set(entries$, parsePropfind(r.bodyText ?? ""));
            } else if (r.status === 404) {
                set(entries$, []);
            } else {
                set(error$, `HTTP ${r.status} ${r.statusText}`.trim());
            }
        })
        .catch((e) => set(error$, errMsg(e)))
        .finally(() => set(loading$, false));
}

export function connect(server: WebDavServer): void {
    set(currentServerId$, server.id);
    set(pathSegments$, []);
    set(view$, "explorer");
    set(error$, null);
    doList(server, []);
}

export function disconnect(): void {
    set(view$, "list");
    set(currentServerId$, null);
    set(entries$, []);
    set(pathSegments$, []);
    set(selectedHref$, null);
    set(error$, null);
}

export function navigateTo(index: number): void {
    const server = get(currentServer$);
    if (!server) return;
    const segs = get(pathSegments$).slice(0, index + 1);
    set(pathSegments$, segs);
    doList(server, segs);
}

/** Navigate back to the current server's root (breadcrumb "Root"). */
export function navigateToRoot(): void {
    const server = get(currentServer$);
    if (!server) return;
    set(pathSegments$, []);
    doList(server, []);
}

export function refresh(): void {
    const server = get(currentServer$);
    if (server) doList(server, get(pathSegments$));
}

export function openFolder(entry: DirEntry): void {
    const server = get(currentServer$);
    if (!server) return;
    const segs = [...get(pathSegments$), entry.name];
    set(pathSegments$, segs);
    doList(server, segs);
}

export function selectEntry(entry: DirEntry): void {
    set(selectedHref$, entry.href);
}

export function doUpload(): void {
    if (!HOST?.pickFile) {
        set(error$, "File picker not available");
        return;
    }
    const server = get(currentServer$);
    if (!server) return;
    const segs = get(pathSegments$);
    HOST.pickFile((picked) => {
        if (!picked) return;
        set(loading$, true);
        set(error$, null);
        http({
            method: "PUT",
            url: joinUrl(server.url, segs) + encodeURIComponent(picked.name),
            username: server.username,
            password: server.password,
            body: picked.bytes,
            responseType: "text",
        })
            .then((r) => {
                if (r.status >= 200 && r.status < 300) doList(server, segs);
                else set(error$, `Upload failed: HTTP ${r.status}`);
            })
            .catch((e) => set(error$, errMsg(e)))
            .finally(() => set(loading$, false));
    });
}

export function doDownload(): void {
    const save = HOST?.saveFile;
    if (!save) {
        set(error$, "Save not available");
        return;
    }
    const entry = get(selectedEntry$);
    if (!entry || entry.isDir) return;
    const server = get(currentServer$);
    if (!server) return;
    const segs = get(pathSegments$);
    set(loading$, true);
    set(error$, null);
    http({
        method: "GET",
        url: joinUrl(server.url, segs) + encodeURIComponent(entry.name),
        username: server.username,
        password: server.password,
        responseType: "bytes",
    })
        .then((r) => {
            if (r.status >= 200 && r.status < 300 && r.bodyBytes) {
                save(entry.name, r.bodyBytes);
            } else {
                set(error$, `Download failed: HTTP ${r.status}`);
            }
        })
        .catch((e) => set(error$, errMsg(e)))
        .finally(() => set(loading$, false));
}

export function requestDelete(entry: DirEntry): void {
    set(confirmDelete$, entry);
}

export function confirmDelete(): void {
    const entry = get(confirmDelete$);
    if (!entry) return;
    set(confirmDelete$, null);
    const server = get(currentServer$);
    if (!server) return;
    const segs = get(pathSegments$);
    set(loading$, true);
    set(error$, null);
    http({
        method: "DELETE",
        url:
            joinUrl(server.url, segs) +
            encodeURIComponent(entry.name) +
            (entry.isDir ? "/" : ""),
        username: server.username,
        password: server.password,
        responseType: "text",
    })
        .then((r) => {
            if (r.status >= 200 && r.status < 300) {
                set(selectedHref$, null);
                doList(server, segs);
            } else {
                set(error$, `Delete failed: HTTP ${r.status}`);
            }
        })
        .catch((e) => set(error$, errMsg(e)))
        .finally(() => set(loading$, false));
}

// --- Connect dialog -------------------------------------------------------

function resetDialogFields(
    name: string,
    url: string,
    user: string,
    pass: string,
): void {
    const span = (s: string) => [{ content: s }];
    nameCtrl.setSpans(span(name));
    urlCtrl.setSpans(span(url));
    userCtrl.setSpans(span(user));
    passCtrl.setSpans(span(pass));
    set(nameDraft$, name);
    set(urlDraft$, url);
    set(userDraft$, user);
    set(passDraft$, pass);
    set(testStatus$, "idle");
    set(testMessage$, "");
}

export function openAddServer(): void {
    set(editingServer$, null);
    resetDialogFields("", "", "", "");
    set(connectOpen$, true);
}

export function openEditServer(server: WebDavServer): void {
    set(editingServer$, server);
    resetDialogFields(
        server.name,
        server.url,
        server.username,
        server.password,
    );
    set(connectOpen$, true);
}

export function closeConnect(): void {
    set(connectOpen$, false);
}

export function runTest(): void {
    const url = get(urlDraft$).trim();
    const user = get(userDraft$).trim();
    const pass = get(passDraft$);
    if (!url) {
        set(testStatus$, "fail");
        set(testMessage$, "Enter a server URL");
        return;
    }
    set(testStatus$, "testing");
    set(testMessage$, "");
    http({
        method: "PROPFIND",
        url,
        username: user,
        password: pass,
        headers: { Depth: "0" },
        responseType: "text",
    })
        .then((r) => {
            if (r.status === 207 || r.status === 200) {
                set(testStatus$, "ok");
                set(testMessage$, `Connected — HTTP ${r.status}`);
            } else if (r.status === 401) {
                set(testStatus$, "fail");
                set(testMessage$, "Authentication failed (401)");
            } else if (r.status === 403) {
                set(testStatus$, "fail");
                set(testMessage$, "Forbidden (403)");
            } else {
                set(testStatus$, "fail");
                set(testMessage$, `HTTP ${r.status} ${r.statusText}`.trim());
            }
        })
        .catch((e) => {
            set(testStatus$, "fail");
            set(testMessage$, errMsg(e));
        });
}

export function saveServer(): void {
    const editing = get(editingServer$);
    const url = get(urlDraft$).trim();
    if (!url) {
        set(testStatus$, "fail");
        set(testMessage$, "Enter a server URL");
        return;
    }
    const name = get(nameDraft$).trim() || url;
    const server: WebDavServer = {
        id: editing ? editing.id : nextId++,
        name,
        url,
        username: get(userDraft$).trim(),
        password: get(passDraft$),
    };
    if (editing) {
        set(
            servers$,
            get(servers$).map((s) => (s.id === editing.id ? server : s)),
        );
    } else {
        set(servers$, [...get(servers$), server]);
    }
    set(connectOpen$, false);
}

export function removeServer(server: WebDavServer): void {
    set(
        servers$,
        get(servers$).filter((s) => s.id !== server.id),
    );
}

// --- New folder dialog ----------------------------------------------------

export function openNewFolder(): void {
    newFolderCtrl.setSpans([{ content: "" }]);
    set(newFolderDraft$, "");
    set(newFolderOpen$, true);
}

export function closeNewFolder(): void {
    set(newFolderOpen$, false);
}

export function submitNewFolder(): void {
    const name = get(newFolderDraft$).trim();
    if (!name) return;
    const server = get(currentServer$);
    if (!server) return;
    const segs = get(pathSegments$);
    set(newFolderOpen$, false);
    set(loading$, true);
    set(error$, null);
    http({
        method: "MKCOL",
        url: joinUrl(server.url, segs) + encodeURIComponent(name),
        username: server.username,
        password: server.password,
        responseType: "text",
    })
        .then((r) => {
            if (r.status >= 200 && r.status < 300) doList(server, segs);
            else set(error$, `Create folder failed: HTTP ${r.status}`);
        })
        .catch((e) => set(error$, errMsg(e)))
        .finally(() => set(loading$, false));
}

export function cancelDelete(): void {
    set(confirmDelete$, null);
}

// ---------------------------------------------------------------------------
// SVG icons — rasterised up front via createSvgResource.
// ---------------------------------------------------------------------------

const ICON_SVGS: Record<string, string> = {
    folder: `<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="#6366f1"><path d="M3 7a2 2 0 0 1 2-2h3.5a2 2 0 0 1 1.5.7l1 1.3H19a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/></svg>`,
    file: `<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="#64748b" stroke-width="2" stroke-linejoin="round" stroke-linecap="round"><path d="M14 3v5h5"/><path d="M14 3H6a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/></svg>`,
    plus: `<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="#ffffff" stroke-width="3" stroke-linecap="round"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>`,
    back: `<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="#475569" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round"><polyline points="15 18 9 12 15 6"/></svg>`,
    refresh: `<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="#475569" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><polyline points="21 4 21 10 15 10"/><path d="M3.5 12a8 8 0 0 1 14-4l3.5 2"/><polyline points="3 20 3 14 9 14"/><path d="M20.5 12a8 8 0 0 1-14 4L3 14"/></svg>`,
};

const iconIds: Record<string, number> = {};
for (const name of Object.keys(ICON_SVGS)) {
    iconIds[name] = createSvgResource(ICON_SVGS[name]);
}

export function getIcon(name: keyof typeof ICON_SVGS): number {
    return iconIds[name];
}
