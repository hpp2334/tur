/**
 * @tur-ng/net — ambient type declarations for the networking module.
 *
 * The runtime is a boa module registered by `tur-net-capability` (via
 * `TurNetPlugin`) under the specifier `"tur:net"` — a JS module wrapping the
 * native bridge fns in the hidden `tur:net/native`. Backends are injected by
 * the embedder (`WasmHttp` in the browser, `NativeHttp` natively,
 * `RecordingHttp` in tests); when none is registered the module is absent
 * and JS code feature-detects.
 */

declare module "tur:net" {
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

    /** Perform an HTTP request. Rejects with `{ message }` on network error. */
    export function request(opts: RequestOptions): Promise<ResponseResult>;

    export interface StreamBody extends AsyncIterable<Uint8Array> {
        /**
         * Read one chunk. Pull-driven: each call polls at most one chunk of
         * network I/O — pausing `next()` IS the backpressure signal. Await
         * each call before the next one; a call made while a previous one is
         * still pending rejects with `{ message }`.
         */
        next(): Promise<IteratorResult<Uint8Array, unknown>>;
        /**
         * Abort the download now (idempotent). The pipe is dropped
         * synchronously — natively the connection closes without waiting for
         * GC. Pending and subsequent `next()` calls resolve
         * `{ done: true }`. (Best-effort on wasm: the browser may keep the
         * connection until GC.)
         */
        cancel(): void;
    }

    export interface StreamResponse {
        ok: true;
        status: number;
        statusText: string;
        headers: Record<string, string>;
        body: StreamBody;
    }

    /**
     * Streaming HTTP request as a **generator coroutine** — `yield*` it from
     * a `launch` generator (a plain call returns an unstarted generator):
     *
     * ```ts
     * launch(function* () {
     *     const resp = yield* requestStream({ url });
     *     let r = yield resp.body.next();
     *     while (!r.done) { /* consume r.value *\/; r = yield resp.body.next(); }
     * });
     * ```
     *
     * The single yield delegates the in-flight promise to the driving
     * `launch`; a network/validation failure throws at the `yield*`
     * (catchable with try/catch — composes with retry/timeout helpers via
     * `yield*`).
     */
    export function requestStream(
        opts: RequestOptions,
    ): Generator<Promise<StreamResponse>, StreamResponse, StreamResponse>;
}
