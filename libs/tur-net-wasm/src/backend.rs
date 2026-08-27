//! `HttpBackend` impl backed by [`reqwest_wasm`]. The `perform_request`
//! helper was extracted verbatim from `tur-wasm/src/app.rs`.

use futures::StreamExt;
use tur_net_capability::{
    HttpBackend, HttpBody, HttpFuture, HttpOutcome, HttpStreamFuture, HttpStreamResponse,
    RequestOpts, ResponseType,
};

/// Browser HTTP backend. Spawns each request through `reqwest_wasm`.
#[derive(Default)]
pub struct WasmHttp;

impl HttpBackend for WasmHttp {
    fn request(&self, opts: RequestOpts) -> HttpFuture {
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

    fn request_stream(&self, opts: RequestOpts) -> HttpStreamFuture {
        // `bufferBytes` is intentionally ignored here: the body is pulled
        // straight from the browser's ReadableStream on each poll (the
        // pull-driven contract), and the browser owns network-level flow
        // control — there is no internal buffering window to size from wasm.
        Box::pin(perform_request_stream(
            opts.url,
            opts.method,
            opts.headers,
            opts.body,
            opts.username,
            opts.password,
        ))
    }
}

pub async fn perform_request(
    url: String,
    method: String,
    headers: Vec<(String, String)>,
    body: Option<HttpBody>,
    want_bytes: bool,
    username: Option<String>,
    password: Option<String>,
) -> HttpOutcome {
    let result: Result<HttpOutcome, String> = async {
        let client = reqwest_wasm::Client::new();
        let m = reqwest_wasm::Method::from_bytes(method.as_bytes())
            .map_err(|e| format!("invalid method {method:?}: {e}"))?;
        let mut rb = client.request(m, &url);
        if let (Some(u), Some(p)) = (username.as_deref(), password.as_deref()) {
            rb = rb.basic_auth(u, Some(p));
        }
        for (k, v) in &headers {
            if let (Ok(name), Ok(val)) = (
                reqwest_wasm::header::HeaderName::from_bytes(k.as_bytes()),
                reqwest_wasm::header::HeaderValue::from_str(v),
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

pub async fn perform_request_stream(
    url: String,
    method: String,
    headers: Vec<(String, String)>,
    body: Option<HttpBody>,
    username: Option<String>,
    password: Option<String>,
) -> Result<HttpStreamResponse, String> {
    let client = reqwest_wasm::Client::new();
    let m = reqwest_wasm::Method::from_bytes(method.as_bytes())
        .map_err(|e| format!("invalid method {method:?}: {e}"))?;
    let mut rb = client.request(m, &url);
    if let (Some(u), Some(p)) = (username.as_deref(), password.as_deref()) {
        rb = rb.basic_auth(u, Some(p));
    }
    for (k, v) in &headers {
        if let (Ok(name), Ok(val)) = (
            reqwest_wasm::header::HeaderName::from_bytes(k.as_bytes()),
            reqwest_wasm::header::HeaderValue::from_str(v),
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
    let body_stream = resp
        .bytes_stream()
        .map(|chunk| chunk.map(|c| c.to_vec()).map_err(|e| format!("{e}")))
        .boxed_local();
    Ok(HttpStreamResponse {
        status,
        status_text,
        headers: hdrs,
        body: body_stream,
    })
}
