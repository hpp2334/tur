/**
 * @tur/host — ambient type declarations for the host-environment module.
 *
 * The runtime is a synthetic boa module registered by tur-wasm
 * (`register_all_services`) under the specifier `"builtin:tur/host"`. It
 * exposes swc-backed compiler services (transpile / tokenize / AST),
 * clipboard access, and file IO (browser picker + download). These live in
 * tur-wasm because they depend on swc and the browser DOM — they are not
 * available in the pure-engine (headless) context.
 */

declare module "builtin:tur/host" {
    // --- swc compiler services -------------------------------------------------

    /** Transpile TSX/TS source to plain JS (throws on parse error). */
    export function transpileTsx(src: string): string;

    /** Lexical token span for syntax highlighting.
     *  `kind` indexes into the caller's highlight palette (see tur-wasm
     *  `highlight_tsx`). */
    export interface TokenSpan {
        start: number;
        end: number;
        kind: number;
    }

    /** Tokenize source into lexical/semantic spans. */
    export function tokenizeTsx(src: string): TokenSpan[];

    /** AST node returned by `generateAst`. Each node includes the exact source
     *  text (`text`) for that declaration, extracted by Rust — so no fragile
     *  position arithmetic on the JS side. For export nodes, `body` contains the
     *  declaration text WITHOUT the `export`/`export default` keyword, also
     *  extracted by Rust from the inner declaration's span. */
    export interface AstNode {
        kind:
            | "import"
            | "exportDecl"
            | "exportDefault"
            | "exportNamed"
            | "exportAll"
            | "exportType"
            | "statement";
        text: string;
        /** For export nodes: text without the `export` keyword. */
        body?: string;
        source?: string;
        specifiers?: Array<{ local: string; imported: string }>;
        names?: string[];
    }

    /** Parse source into top-level AST nodes. */
    export function generateAst(src: string): AstNode[];

    // --- File IO ---------------------------------------------------------------

    /** Result of a successful file pick: file name + raw bytes. */
    export interface PickedFile {
        name: string;
        bytes: ArrayBuffer;
    }

    /** Open the native file picker; `callback` is invoked asynchronously with
     *  `{ name, bytes }` or `null` if cancelled. */
    export function pickFile(
        callback: (result: PickedFile | null) => void,
    ): void;

    /** Trigger a browser download of `bytes` under file name `name`. */
    export function saveFile(name: string, bytes: ArrayBuffer): void;
}
