/**
 * @tur-ng/net — HTTP networking for tur.
 *
 * This module is the consumer-facing surface of `tur-net-capability`. The
 * native bridge fns (`request`, and the promise-based streaming request)
 * live in the internal `tur:net/native` module; this module re-exports
 * `request` unchanged and wraps streaming as a **generator coroutine**.
 *
 * `requestStream` is a `function*`, not an async/promise fn: `yield*` it
 * from a `launch` generator (the engine's coroutine driver — a plain call
 * returns an unstarted generator):
 *
 *     launch(function* () {
 *         const resp = yield* requestStream({ url, bufferBytes: 64 * 1024 });
 *         let r = yield resp.body.next();
 *         while (!r.done) { /* consume r.value (Uint8Array) *\/; r = yield resp.body.next(); }
 *         // or: resp.body.cancel() to abort deterministically
 *     });
 *
 * Being a generator makes streaming composable the way the rest of the
 * engine is: helpers wrap it with `yield*` + try/catch (retries, timeouts)
 * without promise chains, and a failed request throws at the `yield*`,
 * catchable like any other generator error.
 *
 * Backpressure: the body is pulled exactly one chunk per `body.next()` —
 * pausing `next()` pauses the producer (the native bridge honors
 * `bufferBytes` as the max bytes buffered in flight). Await each `next()`
 * before the next call; concurrent calls reject.
 */

import { request, requestStream as nativeRequestStream } from "tur:net/native";

export { request };

/**
 * Streaming HTTP request as a coroutine. Yields once (the internal
 * in-flight promise, delegated to the driving `launch`) and returns the
 * `{ ok, status, statusText, headers, body }` response whose `body` is an
 * async iterable of `Uint8Array` chunks with a `cancel()` method.
 *
 * Must be consumed with `yield*` inside a `launch` generator.
 */
export function* requestStream(opts) {
    return yield nativeRequestStream(opts);
}
