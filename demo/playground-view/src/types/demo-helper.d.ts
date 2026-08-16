/**
 * @tur-ng/demo-helper — ambient type declarations for the demo-helper module.
 *
 * The runtime is a synthetic boa module registered by `tur-playground-plugin`
 * (`TurPlaygroundPlugin`) under the specifier `"tur-ext/demo-helper"`. It
 * exposes swc-backed compiler services (transpile / tokenize / AST). These are
 * playground-only and depend on swc — they are not part of the core engine
 * surface. File IO now lives in `tur:filepicker` (@tur-ng/filepicker).
 */

declare module "tur-ext/demo-helper" {
    // --- swc compiler services -------------------------------------------------

    /** Transpile TSX/TS source to plain JS (throws on parse error). */
    export function transpileTsx(src: string): string;

    /** Lexical token span for syntax highlighting.
     *  `kind` indexes into the caller's highlight palette (see
     *  `highlight_tsx` in tur-playground-plugin). */
    export interface TokenSpan {
        start: number;
        end: number;
        kind: number;
    }

    /** Tokenize source into lexical/semantic spans. */
    export function tokenizeTsx(src: string): TokenSpan[];

    /** AST node returned by `generateAst`. Each node includes the exact source
     *  text (`text`) for that declaration, extracted by Rust — so no fragile
     *  position arithmetic on the JS side. For export nodes, `body` contains
     *  the declaration text WITHOUT the `export`/`export default` keyword,
     *  also extracted by Rust from the inner declaration's span. */
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
}
