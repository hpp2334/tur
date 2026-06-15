export interface CaseInfo {
    name: string;
    files: string[];
    compiledPath: string;
}

const casesMap = new Map<string, CaseInfo>();
const fileContentCache = new Map<string, string>();
const WHITELIST = ["counter", "clickable-text", "container-basic", "column-basic", "todolist"];

let manifestReady = false;

export async function loadManifest(): Promise<void> {
    if (manifestReady) return;
    const resp = await fetch("/cases-manifest.json");
    if (!resp.ok) throw new Error(`Failed to load manifest: ${resp.status}`);
    const manifest: Record<string, string[]> = await resp.json();
    for (const [name, files] of Object.entries(manifest)) {
        if (!WHITELIST.includes(name)) continue;
        casesMap.set(name, {
            name,
            files,
            compiledPath: `/cases/${name}.js`,
        });
    }
    manifestReady = true;
}

export const cases = casesMap;

export function getCaseNames(): string[] {
    return Array.from(casesMap.keys()).sort();
}

export function getCaseFiles(caseName: string): string[] {
    const info = casesMap.get(caseName);
    return info ? info.files : [];
}

export async function fetchFile(
    caseName: string,
    fileName: string,
): Promise<string> {
    const cacheKey = `${caseName}/${fileName}`;
    const cached = fileContentCache.get(cacheKey);
    if (cached) return cached;
    const resp = await fetch(`/sources/${caseName}/${fileName}`);
    if (!resp.ok)
        throw new Error(`failed to fetch ${cacheKey}: ${resp.status}`);
    const source = await resp.text();
    fileContentCache.set(cacheKey, source);
    return source;
}

export async function fetchAllFiles(
    caseName: string,
): Promise<Map<string, string>> {
    const files = getCaseFiles(caseName);
    const result = new Map<string, string>();
    await Promise.all(
        files.map(async (file) => {
            const content = await fetchFile(caseName, file);
            result.set(file, content);
        }),
    );
    return result;
}

export function clearFileCache(caseName: string): void {
    for (const key of fileContentCache.keys()) {
        if (key.startsWith(`${caseName}/`)) {
            fileContentCache.delete(key);
        }
    }
}
