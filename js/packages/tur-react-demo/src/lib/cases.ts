const ctx = require.context(
    "../../../tur-test-cases/react-cases",
    true,
    /\.(ts|tsx)$/,
);

export interface CaseInfo {
    name: string;
    files: string[];
    compiledPath: string;
}

const casesMap = new Map<string, CaseInfo>();
const fileContentCache = new Map<string, string>();

for (const key of ctx.keys()) {
    const match = key.match(/^\.\/([^/]+)\/(.+\.(ts|tsx))$/);
    if (!match) continue;
    const caseName = match[1];
    const fileName = match[2];
    let info = casesMap.get(caseName);
    if (!info) {
        info = {
            name: caseName,
            files: [],
            compiledPath: `/cases/${caseName}.js`,
        };
        casesMap.set(caseName, info);
    }
    info.files.push(fileName);
}

for (const info of casesMap.values()) {
    info.files.sort();
}

export const cases = casesMap;
const WHITELIST = ["todolist", "counter"];

export const caseNames = Array.from(casesMap.keys())
    .filter((name) => WHITELIST.includes(name))
    .sort();

export function getCaseFiles(caseName: string): string[] {
    const info = cases.get(caseName);
    return info ? info.files : [];
}

export async function fetchFile(caseName: string, fileName: string): Promise<string> {
    const cacheKey = `${caseName}/${fileName}`;
    const cached = fileContentCache.get(cacheKey);
    if (cached) return cached;
    const resp = await fetch(`/sources/${caseName}/${fileName}`);
    if (!resp.ok) throw new Error(`failed to fetch ${cacheKey}: ${resp.status}`);
    const source = await resp.text();
    fileContentCache.set(cacheKey, source);
    return source;
}

export async function fetchAllFiles(caseName: string): Promise<Map<string, string>> {
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
