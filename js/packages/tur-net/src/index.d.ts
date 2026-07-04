/**
 * @tur/net — ambient type declarations for the networking module.
 *
 * The runtime is a synthetic boa module registered by tur-wasm
 * (`register_all_services`) under the specifier `"builtin:tur/net"`. It
 * exposes a Promise-based HTTP client backed by reqwest-wasm (wasm-only). Not
 * available in the pure-engine (headless) context.
 */

declare module "builtin:tur/net" {
export interface RequestOptions {
    url: string;
    method?: string;
    headers?: Record<string, string>;
    /** A string or an ArrayBuffer (e.g. from `pickFile`). */
    body?: string | ArrayBuffer;
    /** "text" (default; fills `bodyText`) or "bytes" (fills `bodyBytes`). */
    responseType?: "text" | "bytes";
    username?: string;
    password?: string;
}

export interface ResponseResult {
    ok: true;
    status: number;
    statusText: string;
    headers: Record<string, string>;
    bodyText?: string;
    bodyBytes?: ArrayBuffer;
}

/** Perform an HTTP request. Rejects with `{ message }` on network error. */
export function request(opts: RequestOptions): Promise<ResponseResult>;
}
