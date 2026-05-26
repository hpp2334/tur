const ctx = require.context(
    "../../../tur-test-cases/react-cases",
    true,
    /\/index\.tsx$/,
);

export interface CaseInfo {
    name: string;
    sourcePath: string;
    compiledPath: string;
}

const casesMap = new Map<string, CaseInfo>();

for (const key of ctx.keys()) {
    const match = key.match(/^\.\/([^/]+)\/index\.tsx$/);
    if (!match) continue;
    const name = match[1];
    casesMap.set(name, {
        name,
        sourcePath: `/sources/${name}.tsx`,
        compiledPath: `/cases/${name}.js`,
    });
}

export const cases = casesMap;
export const caseNames = Array.from(casesMap.keys()).sort();

const sourceCache = new Map<string, string>();

export async function fetchSource(name: string): Promise<string> {
    const cached = sourceCache.get(name);
    if (cached) return cached;
    const info = cases.get(name);
    if (!info) throw new Error(`unknown case: ${name}`);
    const resp = await fetch(info.sourcePath);
    if (!resp.ok) throw new Error(`failed to fetch source: ${resp.status}`);
    const source = await resp.text();
    sourceCache.set(name, source);
    return source;
}
