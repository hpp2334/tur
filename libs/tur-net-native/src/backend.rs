//! `HttpBackend` impl backed by native [`reqwest`].
//!
//! reqwest (hyper + hyper-util, DNS via `spawn_blocking`) needs a driven Tokio
//! reactor. That reactor is now provided by the engine's executor
//! (`tur-async`), which wraps a Tokio current-thread runtime + `LocalSet` and
//! is driven on the main thread inside each `flush`/`tick`. So this backend is
//! back to a plain async fn returning a reqwest future — the engine polls it
//! inside a Tokio context, and reqwest's I/O + DNS just work.

use std::future::Future;
use std::pin::Pin;

use tur_net_capability::{HttpBackend, HttpBody, HttpOutcome, RequestOpts, ResponseType};

/// Native HTTP backend. Wraps a `reqwest::Client` (cheap to clone — it
/// reuses a connection pool internally).
#[derive(Default)]
pub struct NativeHttp {
    #[allow(dead_code)]
    _private: (),
}

impl NativeHttp {
    pub fn new() -> Self {
        Self::default()
    }
}

impl HttpBackend for NativeHttp {
    fn request(&self, opts: RequestOpts) -> Pin<Box<dyn Future<Output = HttpOutcome>>> {
        Box::pin(perform_request(
            opts.url,
            opts.method,
            opts.headers,
            opts.body,
            opts.response_type == ResponseType::Bytes,
            opts.username,
            opts.password,
        ))
    }
}

async fn perform_request(
    url: String,
    method: String,
    headers: Vec<(String, String)>,
    body: Option<HttpBody>,
    want_bytes: bool,
    username: Option<String>,
    password: Option<String>,
) -> HttpOutcome {
    let result: Result<HttpOutcome, String> = async {
        let client = reqwest::Client::new();
        let m = reqwest::Method::from_bytes(method.as_bytes())
            .map_err(|e| format!("invalid method {method:?}: {e}"))?;
        let mut rb = client.request(m, &url);
        if let (Some(u), Some(p)) = (username.as_deref(), password.as_deref()) {
            rb = rb.basic_auth(u, Some(p));
        }
        for (k, v) in &headers {
            if let (Ok(name), Ok(val)) = (
                reqwest::header::HeaderName::from_bytes(k.as_bytes()),
                reqwest::header::HeaderValue::from_str(v),
            ) {
                rb = rb.header(name, val);
            }
        }
        rb = match body {
            Some(HttpBody::Text(s)) => rb.body(s),
            Some(HttpBody::Bytes(b)) => rb.body(b),
            None => rb,
        };
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
