/**
 * @tur-ng/net — ambient type declarations for the networking module.
 *
 * The runtime is a boa module registered by `tur-net-capability` (via
 * `TurNetPlugin`) under the specifier `"tur:net"` — both exports are
 * Task-returning native bridge fns. Backends are injected by the embedder
 * (`WasmHttp` in the browser, `NativeHttp` natively, `RecordingHttp` in
 * tests); when none is registered the module is absent and JS code
 * feature-detects.
 *
 * Every async API returns `Task<T> = { promise, cancel() }` (see
 * `tur:std`): await `task.promise`, `task.cancel()` aborts the request and
 * rejects with a `CancelError`.
 */

/// <reference types="@tur-ng/std" />

declare module "tur:net" {
    import type { Task } from "tur:std";

    export interface RequestOptions {
        url: string;
        method?: string;
        headers?: Record<string, string>;
        /** A string or an ArrayBuffer (e.g. from `filePicker.pick()`). */
        body?: string | ArrayBuffer;
        /** "text" (default; fills `bodyText`) or "bytes" (fills `bodyBytes`). */
        responseType?: "text" | "bytes";
        username?: string;
        password?: string;
        /**
         * Streaming only (`requestStream`): max bytes buffered in flight
         * between the network and your `next()` calls — while this much
         * unconsumed data is in flight, the producer pauses (TCP
         * backpressure) and resumes as you pull. Integer `1..=67108864`
         * (64 MiB); default 524288 (512 KiB). Best-effort: the browser
         * (wasm) backend ignores it — the browser owns fetch-body flow
         * control. Ignored by `request`.
         */
        bufferBytes?: number;
    }

    export interface ResponseResult {
        ok: true;
        status: number;
        statusText: string;
        headers: Record<string, string>;
        bodyText?: string;
        bodyBytes?: ArrayBuffer;
    }

    /**
     * Perform an HTTP request. `promise` rejects with `{ message }` on
     * network/validation error; `cancel()` aborts the request (unpolled
     * requests are never sent; in-flight ones are discarded) and rejects
     * with a `CancelError`.
     */
    export function request(opts: RequestOptions): Task<ResponseResult>;

    export interface StreamResponse {
        ok: true;
        status: number;
        statusText: string;
        headers: Record<string, string>;
        /**
         * The response body: a pull-driven `AsyncIterableIterator` yielding
         * `Uint8Array` chunks. Each `next()` polls at most one chunk of
         * network I/O — pausing `next()` IS the backpressure signal. Await
         * each call before the next one; a call made while a previous one
         * is still pending rejects with `{ message }`.
         *
         * There is deliberately no `cancel()` on the body — aborting the
         * download is `task.cancel()` on the `requestStream` Task (see
         * below).
         */
        body: AsyncIterableIterator<Uint8Array>;
    }

    /**
     * Streaming HTTP request:
     *
     * ```ts
     * const t = requestStream({ url, bufferBytes: 64 * 1024 });
     * const resp = await t.promise;
     * for await (const chunk of resp.body) { /* consume chunk *\/ }
     * // t.cancel() anywhere above: pending/subsequent next() resolve
     * // { done: true } so the for-await exits cleanly; if the response
     * // hasn't resolved yet, the promise rejects with a CancelError.
     * ```
     *
     * `cancel()` wire-aborts the download (the response pipe is dropped —
     * natively the connection closes without waiting for GC).
     */
    export function requestStream(opts: RequestOptions): Task<StreamResponse>;
}
