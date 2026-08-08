// Web shim: re-export the namespaces of the *optional* engine modules — those
// the embedder may or may not register. compile.ts sources these via a
// scheme-free alias (`@tur-pg/optional-ns`) so rspack's `tur:`-scheme resolver
// is never reached for builds where the modules are absent. Web builds use this
// shim (the modules stay external — resolved at run time by the engine's boa
// module loader).
import * as FilePicker from "tur:filepicker";
import * as Net from "tur:net";

export { FilePicker, Net };
