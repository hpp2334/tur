// Bootstrap — MUST be imported before anything that calls compileCase.
//
// compileCase() rewrites case imports to `globalThis.TurEdgy`, so that
// assignment must happen before case-store.ts's cache priming runs. Because
// ES modules evaluate all imports before the importer's body, we can't put
// this in index.ts directly — it needs to be a side-effect of the first
// imported module.

import * as Edgy from "@tur/edgy";

(globalThis as unknown as { TurEdgy: unknown }).TurEdgy = Edgy;
