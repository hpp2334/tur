//! `HttpBackend` impl backed by native [`reqwest`], running on a
//! **user-provided** tokio runtime.
//!
//! reqwest (hyper + hyper-util, DNS via `spawn_blocking`) needs a driven tokio
//! reactor. The engine is tokio-free — it does not build, own, or enter a tokio
//! runtime. Instead, [`NativeHttp`] holds a [`tokio::runtime::Handle`] supplied
//! by the embedder; each request is `spawn`ed onto that runtime, and the result
//! is bridged back to the engine's main-thread executor via a
//! reactor-agnostic channel (`oneshot` for [`request`](HttpBackend::request),
//! `mpsc` for [`request_stream`](HttpBackend::request_stream)).
//!
//! The bridge future (`oneshot::Receiver` / `mpsc::Receiver` polled via the
//! engine's hand-rolled executor) does not require a tokio context — it is a
//! plain channel that registers `cx.waker()`. When the spawned tokio task
//! completes, it sends the result + wakes that waker, which the engine's next
//! `tick()` observes.
//!
//! Streaming responses are **byte-budgeted** (backpressured): a
//! `tokio::sync::Semaphore` shared between the producer task and the
//! engine-side [`MpscBodyStream`] caps the bytes in flight between the socket
//! read and the JS consumer (see [`pump_body`]). While the budget is consumed
//! the producer parks, reqwest stops reading, and TCP flow control closes the
//! window mid-body.
//!
//! ## User contract
//!
//! The embedder builds and owns a tokio runtime for the engine's lifetime,
//! then passes a (cheaply-cloned) [`Handle`] to [`NativeHttp::new`]:
//!
//! ```no_run
//! # use tur_net_native::{Http, NativeHttp};
//! # fn user_tokio_handle() -> tokio::runtime::Handle { unimplemented!() }
//! // The embedder owns the runtime (built with whatever tokio features it
//! // needs — e.g. `rt-multi-thread` so worker threads drive the reactor).
//! let handle: tokio::runtime::Handle = user_tokio_handle();
//! let http_backend = NativeHttp::new(handle);
//! let _http_cap = Http::new(http_backend);
//! // Register on the engine runtime builder:
//! //   builder.capability(_http_cap).plugin(TurNetPlugin)
//! // The handle's runtime must outlive the engine runtime — drop the engine
//! // before the tokio runtime.
//! ```

use std::sync::Arc;
use std::task::Context;
use std::task::Poll;

use futures::Stream;
use futures::StreamExt;
use futures::stream::BoxStream;
use tokio::sync::Semaphore;
use tur_net_capability::{
    HttpBackend, HttpBody, HttpFuture, HttpOutcome, HttpStreamFuture, HttpStreamResponse,
    RequestOpts, ResponseType,
};

/// In-flight byte budget between the socket read and the JS consumer when a
/// request omits `bufferBytes`. The JS side can raise/lower it per request
/// via `requestStream({ bufferBytes })` (validated by the `tur:net` bridge
/// to `1..=64 MiB`). 512 KiB ≈ 8 × 64 KiB chunks — enough to keep one chunk
/// always ready for the next `body.next()` while a slow consumer pauses the
/// download mid-body.
pub const DEFAULT_STREAM_BUFFER_BYTES: usize = 512 * 1024;

/// Carrier-channel capacity, in **chunks**. This is a defense-in-depth
/// backstop only — the real bound is the byte budget semaphore (see
/// [`pump_body`]); the channel can never hold more than `budget` bytes
/// because every byte in it was acquired from the semaphore first.
const STREAM_CHANNEL_CHUNK_BACKSTOP: usize = 1024;

/// Native HTTP backend. Holds a [`tokio::runtime::Handle`] (cloned from a
/// runtime the embedder owns) and `spawn`s each request onto it.
///
/// The engine never builds or enters a tokio runtime itself; the embedder is
/// responsible for keeping the runtime alive for at least as long as this
/// backend (and the [`TurRuntime`](tur_engine::TurRuntime) it's registered on)
/// is alive.
pub struct NativeHttp {
    handle: tokio::runtime::Handle,
}

impl NativeHttp {
    /// Wrap a handle to an embedder-owned tokio runtime. The handle is cheap
    /// to clone; the runtime it points to must outlive this backend.
    pub fn new(handle: tokio::runtime::Handle) -> Self {
        Self { handle }
    }
}

impl From<tokio::runtime::Handle> for NativeHttp {
    fn from(handle: tokio::runtime::Handle) -> Self {
        Self::new(handle)
    }
}

impl HttpBackend for NativeHttp {
    fn request(&self, opts: RequestOpts) -> HttpFuture {
        let handle = self.handle.clone();
        Box::pin(async move {
            let (tx, rx) = tokio::sync::oneshot::channel::<HttpOutcome>();
            handle.spawn(async move {
                let outcome = perform_request(opts).await;
                let _ = tx.send(outcome);
            });
            // `oneshot::Receiver` is reactor-agnostic: it polls as a plain
            // channel that registers `cx.waker()`, so the engine's
            // hand-rolled executor drives it without a tokio context.
            match rx.await {
                Ok(outcome) => outcome,
                Err(_) => HttpOutcome::Err("http task was dropped".to_string()),
            }
        })
    }

    fn request_stream(&self, opts: RequestOpts) -> HttpStreamFuture {
        let handle = self.handle.clone();
        Box::pin(async move {
            let budget = resolve_stream_budget(&opts);
            let permits = Arc::new(Semaphore::new(budget));
            let (tx, rx) = stream_channel();
            let producer_permits = permits.clone();
            handle.spawn(async move {
                run_stream_request(opts, tx, producer_permits, budget).await;
            });
            let mut rx = MpscBodyStream { rx, permits };
            // First message carries the response headers (or an error).
            match rx.recv_once().await {
                StreamMsgOnce::Headers {
                    status,
                    status_text,
                    headers,
                } => Ok(HttpStreamResponse {
                    status,
                    status_text,
                    headers,
                    body: rx.boxed_local(),
                }),
                StreamMsgOnce::Error(e) => Err(e),
                StreamMsgOnce::Closed => Err("stream closed before headers".to_string()),
                StreamMsgOnce::ChunkBeforeHeaders => {
                    Err("protocol: chunk before headers".to_string())
                }
            }
        })
    }
}

/// Effective in-flight byte budget for one streaming request: the caller's
/// `bufferBytes` (already validated by the bridge to `1..=64 MiB`) or the
/// module default. Never below 1.
fn resolve_stream_budget(opts: &RequestOpts) -> usize {
    opts.stream_buffer_bytes
        .map(|b| b as usize)
        .unwrap_or(DEFAULT_STREAM_BUFFER_BYTES)
        .max(1)
}

/// The chunk-carrier channel. The capacity is a chunk-count backstop only —
/// the byte budget semaphore is the real bound (see [`pump_body`]).
fn stream_channel() -> (
    tokio::sync::mpsc::Sender<StreamMsg>,
    tokio::sync::mpsc::Receiver<StreamMsg>,
) {
    tokio::sync::mpsc::channel(STREAM_CHANNEL_CHUNK_BACKSTOP)
}

/// Message exchanged between the tokio-spawned request task and the
/// engine-side receiver. Must be `Send` (crosses from a tokio worker thread
/// to the engine's main thread); all variants are.
enum StreamMsg {
    Headers {
        status: u16,
        status_text: String,
        headers: Vec<(String, String)>,
    },
    Chunk(Vec<u8>),
    Error(String),
}

/// Single-shot result of awaiting the first message on the body stream.
enum StreamMsgOnce {
    Headers {
        status: u16,
        status_text: String,
        headers: Vec<(String, String)>,
    },
    /// A body chunk arrived before headers — a protocol violation.
    ChunkBeforeHeaders,
    Error(String),
    Closed,
}

/// Adapts a tokio `mpsc::Receiver<StreamMsg>` into a
/// `Stream<Item = Result<Vec<u8>, String>>` that the engine polls with its own
/// (tokio-free) executor. `poll_recv` is reactor-agnostic — it just registers
/// `cx.waker()`, which the sender wakes on the tokio worker thread.
///
/// Also carries the consumer half of the byte-budget gate: each chunk that
/// leaves the pipe re-credits its bytes to the shared [`Semaphore`] so a
/// parked producer (see [`pump_body`]) resumes. Dropping this stream closes
/// the semaphore, which fails the producer's pending `acquire_many` — the
/// abort path for "JS dropped/cancelled the body".
struct MpscBodyStream {
    rx: tokio::sync::mpsc::Receiver<StreamMsg>,
    permits: Arc<Semaphore>,
}

impl MpscBodyStream {
    /// Await the next message (the first one — headers or error). Uses
    /// `recv()` (which is also reactor-agnostic) for symmetry; the subsequent
    /// body chunks are polled via the [`Stream`] impl below.
    async fn recv_once(&mut self) -> StreamMsgOnce {
        match self.rx.recv().await {
            None => StreamMsgOnce::Closed,
            Some(StreamMsg::Headers {
                status,
                status_text,
                headers,
            }) => StreamMsgOnce::Headers {
                status,
                status_text,
                headers,
            },
            Some(StreamMsg::Chunk(_)) => StreamMsgOnce::ChunkBeforeHeaders,
            Some(StreamMsg::Error(e)) => StreamMsgOnce::Error(e),
        }
    }
}

impl Drop for MpscBodyStream {
    fn drop(&mut self) {
        // Wake a producer parked on the byte budget: closing the semaphore
        // fails its pending `acquire_many`, so the pump aborts instead of
        // hanging onto the connection. (A producer parked on the carrier
        // channel's capacity is covered by that channel's own send-error
        // path.)
        self.permits.close();
    }
}

impl Stream for MpscBodyStream {
    type Item = Result<Vec<u8>, String>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        match self.rx.poll_recv(cx) {
            Poll::Ready(Some(StreamMsg::Chunk(v))) => {
                // The chunk just left the pipe — hand its bytes back to the
                // producer's budget (this is what resumes a parked producer).
                self.permits.add_permits(v.len());
                Poll::Ready(Some(Ok(v)))
            }
            Poll::Ready(Some(StreamMsg::Error(e))) => Poll::Ready(Some(Err(e))),
            // Headers were already consumed by `recv_once` before the stream
            // is handed out; treat any subsequent/odd message as end-of-stream.
            Poll::Ready(Some(StreamMsg::Headers { .. })) => Poll::Ready(None),
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

// ---------------------------------------------------------------------------
// Request execution (runs on the user's tokio runtime via `handle.spawn`)
// ---------------------------------------------------------------------------

async fn perform_request(opts: RequestOpts) -> HttpOutcome {
    let result: Result<HttpOutcome, String> = async {
        let client = reqwest::Client::new();
        let m = reqwest::Method::from_bytes(opts.method.as_bytes())
            .map_err(|e| format!("invalid method {:?}: {e}", opts.method))?;
        let mut rb = client.request(m, &opts.url);
        if let (Some(u), Some(p)) = (opts.username.as_deref(), opts.password.as_deref()) {
            rb = rb.basic_auth(u, Some(p));
        }
        for (k, v) in &opts.headers {
            if let (Ok(name), Ok(val)) = (
                reqwest::header::HeaderName::from_bytes(k.as_bytes()),
                reqwest::header::HeaderValue::from_str(v),
            ) {
                rb = rb.header(name, val);
            }
        }
        rb = match opts.body {
            Some(HttpBody::Text(s)) => rb.body(s),
            Some(HttpBody::Bytes(b)) => rb.body(b),
            None => rb,
        };
        let want_bytes = opts.response_type == ResponseType::Bytes;
        let resp = rb.send().await.map_err(|e| format!("{e}"))?;
        let status = resp.status().as_u16();
        let status_text = resp.status().canonical_reason().unwrap_or("").to_string();
        let hdrs: Vec<(String, String)> = resp
            .headers()
            .iter()
            .map(|(k, v)| (k.as_str().to_string(), v.to_str().unwrap_or("").to_string()))
            .collect();
        let body = if want_bytes {
            HttpBody::Bytes(resp.bytes().await.map_err(|e| format!("{e}"))?.to_vec())
        } else {
            HttpBody::Text(resp.text().await.map_err(|e| format!("{e}"))?)
        };
        Ok(HttpOutcome::Ok {
            status,
            status_text,
            headers: hdrs,
            body,
        })
    }
    .await;
    result.unwrap_or_else(HttpOutcome::Err)
}

/// Drive a streaming request on the user's tokio runtime. Sends headers (or an
/// error) as the first [`StreamMsg`], then forwards body chunks under the byte
/// budget until the response stream ends or errors. The channel closes
/// (dropping `tx`) when this returns, which the engine-side `Stream` observes
/// as end-of-stream.
async fn run_stream_request(
    opts: RequestOpts,
    tx: tokio::sync::mpsc::Sender<StreamMsg>,
    permits: Arc<Semaphore>,
    budget: usize,
) {
    let client = reqwest::Client::new();
    let m = match reqwest::Method::from_bytes(opts.method.as_bytes()) {
        Ok(m) => m,
        Err(e) => {
            let _ = tx
                .send(StreamMsg::Error(format!(
                    "invalid method {:?}: {e}",
                    opts.method
                )))
                .await;
            return;
        }
    };
    let mut rb = client.request(m, &opts.url);
    if let (Some(u), Some(p)) = (opts.username.as_deref(), opts.password.as_deref()) {
        rb = rb.basic_auth(u, Some(p));
    }
    for (k, v) in &opts.headers {
        if let (Ok(name), Ok(val)) = (
            reqwest::header::HeaderName::from_bytes(k.as_bytes()),
            reqwest::header::HeaderValue::from_str(v),
        ) {
            rb = rb.header(name, val);
        }
    }
    rb = match opts.body {
        Some(HttpBody::Text(s)) => rb.body(s),
        Some(HttpBody::Bytes(b)) => rb.body(b),
        None => rb,
    };
    let resp = match rb.send().await {
        Ok(r) => r,
        Err(e) => {
            let _ = tx.send(StreamMsg::Error(format!("{e}"))).await;
            return;
        }
    };
    let status = resp.status().as_u16();
    let status_text = resp.status().canonical_reason().unwrap_or("").to_string();
    let hdrs: Vec<(String, String)> = resp
        .headers()
        .iter()
        .map(|(k, v)| (k.as_str().to_string(), v.to_str().unwrap_or("").to_string()))
        .collect();
    // Headers ride outside the byte budget (one small message, sent before
    // any chunk — the channel is empty here, so this never parks). The body
    // below is what the budget bounds.
    let _ = tx
        .send(StreamMsg::Headers {
            status,
            status_text,
            headers: hdrs,
        })
        .await;
    let body = resp
        .bytes_stream()
        .map(|chunk| chunk.map(|c| c.to_vec()).map_err(|e| format!("{e}")))
        .boxed();
    pump_body(body, tx, permits, budget).await;
}

/// Forward body chunks into the carrier channel under the byte budget.
///
/// **Backpressure**: each piece first [`Semaphore::acquire_many`]s its byte
/// count from the shared budget (1 permit = 1 byte) and *forgets* the permit —
/// the bytes return only when the consumer polls the piece out of the pipe
/// ([`MpscBodyStream`]'s [`Stream`] impl re-credits them). While `budget`
/// bytes sit unconsumed this pump parks on `acquire_many`, reqwest stops
/// reading, and TCP flow control closes the window mid-body. The carrier
/// channel's capacity is a chunk-count backstop only.
///
/// A chunk larger than the whole budget could never satisfy `acquire_many`
/// (permits available never exceed `budget`), so oversized chunks are
/// delivered as budget-sized pieces — chunk boundaries were never contractual.
async fn pump_body(
    mut src: BoxStream<'static, Result<Vec<u8>, String>>,
    tx: tokio::sync::mpsc::Sender<StreamMsg>,
    permits: Arc<Semaphore>,
    budget: usize,
) {
    while let Some(chunk) = src.next().await {
        match chunk {
            Ok(c) => {
                if c.is_empty() {
                    continue;
                }
                if c.len() <= budget {
                    if !send_budgeted(&tx, &permits, c).await {
                        return;
                    }
                } else {
                    for start in (0..c.len()).step_by(budget) {
                        let end = (start + budget).min(c.len());
                        if !send_budgeted(&tx, &permits, c[start..end].to_vec()).await {
                            return;
                        }
                    }
                }
            }
            Err(e) => {
                let _ = tx.send(StreamMsg::Error(e)).await;
                return;
            }
        }
    }
}

/// Acquire `piece`'s worth of byte budget (parking the producer while the
/// consumer is `budget` bytes behind), then send it. Returns `false` when the
/// pump should abort: the receiver was dropped, or the gate was closed under
/// us (the consumer side dropped the stream).
async fn send_budgeted(
    tx: &tokio::sync::mpsc::Sender<StreamMsg>,
    permits: &Semaphore,
    piece: Vec<u8>,
) -> bool {
    let n = u32::try_from(piece.len()).unwrap_or(u32::MAX);
    match permits.acquire_many(n).await {
        Ok(permit) => permit.forget(),
        Err(_) => return false, // gate closed — consumer went away
    }
    tx.send(StreamMsg::Chunk(piece)).await.is_ok()
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use futures::StreamExt;
    use futures::stream;
    use tokio::sync::Semaphore;

    use super::{MpscBodyStream, pump_body, stream_channel};

    /// A lazy source of `n` chunks × `size` bytes that counts how many
    /// chunks have been pulled from it (a pull happens exactly when the pump
    /// polls — `stream::iter` is lazy).
    fn counting_source(
        n: usize,
        size: usize,
        pulled: Arc<AtomicUsize>,
    ) -> futures::stream::BoxStream<'static, Result<Vec<u8>, String>> {
        stream::iter((0..n).map(move |i| {
            pulled.fetch_add(1, Ordering::SeqCst);
            Ok(vec![i as u8; size])
        }))
        .boxed()
    }

    fn test_runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("test runtime")
    }

    /// The headline backpressure pin: with the consumer stalled, the producer
    /// parks after ~`budget` bytes in flight instead of draining the whole
    /// body into the bridge. Each consumed chunk re-credits exactly its own
    /// bytes, and the stream stays lossless to the end.
    #[test]
    fn producer_parks_when_consumer_slow() {
        const N: usize = 500;
        const SIZE: usize = 10;
        const BUDGET: usize = 100; // 10 chunks' worth

        let pulled = Arc::new(AtomicUsize::new(0));
        let src = counting_source(N, SIZE, pulled.clone());

        let rt = test_runtime();
        let permits = Arc::new(Semaphore::new(BUDGET));
        let (tx, rx) = stream_channel();
        let mut body = MpscBodyStream {
            rx,
            permits: permits.clone(),
        };

        // Spawn the pump, then let it run to its park point (no consumer polls).
        let task = rt
            .handle()
            .spawn(pump_body(src, tx, permits.clone(), BUDGET));
        rt.block_on(async {
            for _ in 0..200 {
                tokio::task::yield_now().await;
            }
        });

        // Parked: budgeted chunks buffered + at most one chunk in the
        // producer's hand (it pulls before it acquires).
        let parked_at = pulled.load(Ordering::SeqCst);
        assert!(
            parked_at <= BUDGET / SIZE + 1,
            "producer must park when the consumer stalls: pulled {parked_at}"
        );
        assert!(parked_at < N);

        // Consuming one chunk releases exactly its bytes back — the producer
        // advances by exactly one chunk, then parks again.
        let first = rt
            .block_on(body.next())
            .expect("chunk")
            .expect("ok");
        assert_eq!(first.len(), SIZE);
        rt.block_on(async {
            for _ in 0..200 {
                tokio::task::yield_now().await;
            }
        });
        assert_eq!(pulled.load(Ordering::SeqCst), parked_at + 1);

        // ...and the stream is lossless + in order to the end.
        let mut total = first.len();
        let mut next_byte = first[0] + 1; // the drain loop starts at chunk 1
        rt.block_on(async {
            while let Some(item) = body.next().await {
                let chunk = item.expect("ok");
                assert_eq!(chunk[0], next_byte, "chunks must stay in order");
                next_byte = next_byte.wrapping_add(1);
                total += chunk.len();
            }
        });
        assert_eq!(total, N * SIZE);
        rt.block_on(task).expect("pump task panicked");
    }

    /// A chunk larger than the whole budget must still make progress (split
    /// into budget-sized pieces) and stay byte-exact — the oversized-acquire
    /// deadlock guard.
    #[test]
    fn oversized_chunks_split_to_budget_and_stay_lossless() {
        const N: usize = 5;
        const SIZE: usize = 20;
        const BUDGET: usize = 8; // < one chunk

        let src = stream::iter((0..N).map(|i| Ok(vec![i as u8; SIZE]))).boxed();
        let rt = test_runtime();
        let permits = Arc::new(Semaphore::new(BUDGET));
        let (tx, rx) = stream_channel();
        let mut body = MpscBodyStream {
            rx,
            permits: permits.clone(),
        };

        rt.block_on(async {
            let task = tokio::spawn(pump_body(src, tx, permits.clone(), BUDGET));
            let mut total = 0usize;
            while let Some(item) = body.next().await {
                let piece = item.expect("ok");
                assert!(
                    piece.len() <= BUDGET,
                    "pieces must be budget-sized: {}",
                    piece.len()
                );
                total += piece.len();
            }
            task.await.expect("pump task panicked");
            assert_eq!(total, N * SIZE);
        });
    }

    /// Dropping the consumer must abort the producer promptly (semaphore
    /// closed → pending `acquire_many` fails), not leak it holding the
    /// connection.
    #[test]
    fn producer_aborts_when_consumer_dropped() {
        const N: usize = 500;
        const SIZE: usize = 10;
        const BUDGET: usize = 4096; // binds before the chunk backstop

        let pulled = Arc::new(AtomicUsize::new(0));
        let src = counting_source(N, SIZE, pulled.clone());

        let rt = test_runtime();
        let permits = Arc::new(Semaphore::new(BUDGET));
        let (tx, rx) = stream_channel();
        let body = MpscBodyStream {
            rx,
            permits: permits.clone(),
        };

        let task = rt.handle().spawn(pump_body(src, tx, permits, BUDGET));
        rt.block_on(async {
            for _ in 0..200 {
                tokio::task::yield_now().await;
            }
        });
        let pulled_before_drop = pulled.load(Ordering::SeqCst);
        assert!(pulled_before_drop < N, "producer should be parked on budget");

        drop(body); // closes the semaphore + drops the receiver
        rt.block_on(async {
            for _ in 0..200 {
                tokio::task::yield_now().await;
            }
        });
        assert!(task.is_finished(), "producer must abort after consumer drop");
        let pulled_after = pulled.load(Ordering::SeqCst);
        assert!(pulled_after < N);
        let _ = rt.block_on(task);
    }
}

