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

use std::task::Context;
use std::task::Poll;

use futures::Stream;
use futures::StreamExt;
use tur_net_capability::{
    HttpBackend, HttpBody, HttpFuture, HttpOutcome, HttpStreamFuture, HttpStreamResponse,
    RequestOpts, ResponseType,
};

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
            let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<StreamMsg>();
            handle.spawn(async move {
                run_stream_request(opts, tx).await;
            });
            let mut rx = MpscBodyStream(rx);
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

/// Adapts a tokio `mpsc::UnboundedReceiver<StreamMsg>` into a
/// `Stream<Item = Result<Vec<u8>, String>>` that the engine polls with its own
/// (tokio-free) executor. `poll_recv` is reactor-agnostic — it just registers
/// `cx.waker()`, which the sender wakes on the tokio worker thread.
struct MpscBodyStream(tokio::sync::mpsc::UnboundedReceiver<StreamMsg>);

impl MpscBodyStream {
    /// Await the next message (the first one — headers or error). Uses
    /// `recv()` (which is also reactor-agnostic) for symmetry; the subsequent
    /// body chunks are polled via the [`Stream`] impl below.
    async fn recv_once(&mut self) -> StreamMsgOnce {
        match self.0.recv().await {
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

impl Stream for MpscBodyStream {
    type Item = Result<Vec<u8>, String>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        match self.0.poll_recv(cx) {
            Poll::Ready(Some(StreamMsg::Chunk(v))) => Poll::Ready(Some(Ok(v))),
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
/// error) as the first [`StreamMsg`], then forwards body chunks until the
/// response stream ends or errors. The channel closes (dropping `tx`) when
/// this returns, which the engine-side `Stream` observes as end-of-stream.
async fn run_stream_request(opts: RequestOpts, tx: tokio::sync::mpsc::UnboundedSender<StreamMsg>) {
    let client = reqwest::Client::new();
    let m = match reqwest::Method::from_bytes(opts.method.as_bytes()) {
        Ok(m) => m,
        Err(e) => {
            let _ = tx.send(StreamMsg::Error(format!(
                "invalid method {:?}: {e}",
                opts.method
            )));
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
            let _ = tx.send(StreamMsg::Error(format!("{e}")));
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
    let _ = tx.send(StreamMsg::Headers {
        status,
        status_text,
        headers: hdrs,
    });

    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(c) => {
                if tx.send(StreamMsg::Chunk(c.to_vec())).is_err() {
                    // Receiver dropped (engine dropped the response stream) — stop.
                    break;
                }
            }
            Err(e) => {
                let _ = tx.send(StreamMsg::Error(format!("{e}")));
                break;
            }
        }
    }
}
