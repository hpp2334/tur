//! End-to-end test for the `webdav-client` case against an in-process fake
//! WebDAV "server".
//!
//! The real `__tur.request` lives in tur-wasm (browser fetch). Under the native
//! test engine we register a synchronous stand-in backed by a shared in-memory
//! filesystem (`FakeFs`) — a fake WebDAV server. It resolves the returned
//! Promise immediately (fulfilled), so the case's `.then` bodies run on the
//! next flush just like in the browser. This lets us exercise the case's real
//! connect / browse / PROPFIND-parse / layout paths (and the file IO host fns)
//! without a browser or network.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use boa_engine::native_function::NativeFunction;
use boa_engine::object::builtins::JsPromise;
use boa_engine::object::JsObject;
use boa_engine::{js_string, JsArgs, JsValue};
use tur_engine::core::elements::TraceValue;
use tur_engine::TurApp;
use tur_integration_tests::TurTestApp;

// ---------------------------------------------------------------------------
// Fake in-memory WebDAV filesystem
// ---------------------------------------------------------------------------

#[derive(Clone)]
enum Node {
    Dir,
    File(Vec<u8>),
}

#[derive(Default)]
struct FakeFs {
    // path ("/dav/") -> node, ordered so PROPFIND children are stable
    entries: BTreeMap<String, Node>,
}

impl FakeFs {
    fn seed() -> Self {
        let mut e = BTreeMap::new();
        e.insert("/dav/".to_string(), Node::Dir);
        e.insert("/dav/Documents/".to_string(), Node::Dir);
        e.insert(
            "/dav/welcome.txt".to_string(),
            Node::File(b"Hello from tur WebDAV!\n".to_vec()),
        );
        Self { entries: e }
    }

    /// Direct children of `dir` (which must end in "/").
    fn children(&self, dir: &str) -> Vec<(String, Node)> {
        self.entries
            .iter()
            .filter(|(k, _)| k.as_str() != dir && k.starts_with(dir))
            .filter(|(k, _)| {
                let rest = &k[dir.len()..];
                // exactly one path component (allow a single trailing "/")
                let trimmed = rest.trim_end_matches('/');
                !trimmed.is_empty() && !trimmed.contains('/')
            })
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }
}

/// Extract the path component from a URL like "http://host/dav/Documents/".
fn url_path(url: &str) -> String {
    let after_scheme = match url.split_once("://") {
        Some((_, rest)) => rest,
        None => url,
    };
    match after_scheme.find('/') {
        Some(i) => after_scheme[i..].to_string(),
        None => "/".to_string(),
    }
}

/// Build a WebDAV multistatus body for a PROPFIND on `dir`.
fn multistatus(fs: &FakeFs, dir: &str) -> String {
    let mut xml = String::from("<?xml version=\"1.0\" encoding=\"utf-8\"?>");
    xml.push_str("<D:multistatus xmlns:D=\"DAV:\">");
    // self first — the case parser skips the first <response>.
    xml.push_str(&response_xml(dir, true, 0));
    for (path, node) in fs.children(dir) {
        let (is_dir, size) = match node {
            Node::Dir => (true, 0),
            Node::File(b) => (false, b.len()),
        };
        xml.push_str(&response_xml(&path, is_dir, size));
    }
    xml.push_str("</D:multistatus>");
    xml
}

fn response_xml(href: &str, is_dir: bool, size: usize) -> String {
    format!(
        "<D:response><D:href>{href}</D:href>\
         <D:propstat><D:prop>\
         <D:resourcetype>{collection}</D:resourcetype>\
         <D:getcontentlength>{size}</D:getcontentlength>\
         <D:getlastmodified>Mon, 01 Jan 2024 00:00:00 GMT</D:getlastmodified>\
         </D:prop>\
         <D:status>HTTP/1.1 200 OK</D:status>\
         </D:propstat></D:response>",
        collection = if is_dir { "<D:collection/>" } else { "" }
    )
}

// ---------------------------------------------------------------------------
// Register fakes: `__tur.request` + `__turHost.pickFile` / `saveFile`
// ---------------------------------------------------------------------------

/// Recorded file saves (name -> bytes) performed via `__turHost.saveFile`.
type SaveLog = Rc<RefCell<Vec<(String, Vec<u8>)>>>;

fn js_str(obj: &JsObject, key: &str, ctx: &mut boa_engine::Context) -> Option<String> {
    obj.get(js_string!(key), ctx)
        .ok()
        .and_then(|v| v.as_string().map(|s| s.to_std_string_escaped()))
}

fn register_webdav_fakes(app: &mut TurApp, fs: Rc<RefCell<FakeFs>>) -> SaveLog {
    let fs_req = fs.clone();
    let request = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx| {
            let opts = args.get_or_undefined(0);
            let obj = opts.as_object();
            let Some(obj) = obj else {
                let p = JsPromise::reject(
                    boa_engine::JsError::from(boa_engine::JsNativeError::typ().with_message(
                        "request: options object required",
                    )),
                    ctx,
                )?;
                return Ok(p.into());
            };

            let url = js_str(&obj, "url", ctx).unwrap_or_default();
            let method = js_str(&obj, "method", ctx).unwrap_or_else(|| "GET".to_string());
            let response_type =
                js_str(&obj, "responseType", ctx).unwrap_or_else(|| "text".to_string());

            let path = url_path(&url);
            let (status, status_text, text_body, bytes_body) = {
                let fs = fs_req.borrow();
                match method.as_str() {
                    "PROPFIND" => {
                        let xml = multistatus(&fs, &path);
                        (207u16, "Multi-Status".to_string(), Some(xml), None)
                    }
                    "MKCOL" => (201, "Created".to_string(), None, None),
                    "PUT" => (201, "Created".to_string(), None, None),
                    "GET" => match fs.entries.get(&path) {
                        Some(Node::File(b)) => (200, "OK".to_string(), None, Some(b.clone())),
                        _ => (404, "Not Found".to_string(), None, None),
                    },
                    "DELETE" => (204, "No Content".to_string(), None, None),
                    _ => (405, "Method Not Allowed".to_string(), None, None),
                }
            };

            let ok = (200..300).contains(&status);
            let resp = JsObject::with_object_proto(ctx.intrinsics());
            let _ = resp.create_data_property(js_string!("ok"), JsValue::from(ok), ctx);
            let _ =
                resp.create_data_property(js_string!("status"), JsValue::from(status as f64), ctx);
            let _ = resp.create_data_property(
                js_string!("statusText"),
                JsValue::from(js_string!(status_text.as_str())),
                ctx,
            );
            let headers = JsObject::with_object_proto(ctx.intrinsics());
            let _ = resp.create_data_property(js_string!("headers"), JsValue::from(headers), ctx);
            if response_type == "bytes" {
                if let Some(b) = bytes_body {
                    use boa_engine::object::builtins::{AlignedVec, JsArrayBuffer};
                    if let Ok(ab) = JsArrayBuffer::from_byte_block(
                        AlignedVec::from_iter(0, b),
                        ctx,
                    ) {
                        let _ = resp.create_data_property(
                            js_string!("bodyBytes"),
                            JsValue::from(ab),
                            ctx,
                        );
                    }
                }
            } else if let Some(t) = text_body {
                let _ = resp.create_data_property(
                    js_string!("bodyText"),
                    JsValue::from(js_string!(t.as_str())),
                    ctx,
                );
            }

            let p = JsPromise::resolve(resp, ctx)?;
            Ok(p.into())
        })
    };
    app.register_tur_fn("request", 1, request)
        .expect("register __tur.request");

    // pickFile — immediately invoke the callback with a canned file.
    let pick_file = NativeFunction::from_copy_closure(|_this, args, ctx| {
        use boa_engine::object::builtins::{AlignedVec, JsArrayBuffer, JsFunction};
        let Some(cb_obj) = args.get_or_undefined(0).as_object() else {
            return Ok(JsValue::undefined());
        };
        let Some(cb) = JsFunction::from_object(cb_obj.clone()) else {
            return Ok(JsValue::undefined());
        };
        let payload = b"uploaded-via-test".to_vec();
        let o = JsObject::with_object_proto(ctx.intrinsics());
        let _ = o.create_data_property(
            js_string!("name"),
            JsValue::from(js_string!("upload-test.txt")),
            ctx,
        );
        if let Ok(ab) =
            JsArrayBuffer::from_byte_block(AlignedVec::from_iter(0, payload), ctx)
        {
            let _ = o.create_data_property(js_string!("bytes"), JsValue::from(ab), ctx);
        }
        let _ = cb.call(&JsValue::undefined(), &[o.into()], ctx);
        Ok(JsValue::undefined())
    });
    app.register_host_fn("pickFile", 1, pick_file)
        .expect("register pickFile");

    // saveFile — record (name, bytes) for later assertion.
    let save_log: SaveLog = Rc::new(RefCell::new(Vec::new()));
    let save_file = {
        let log = save_log.clone();
        unsafe {
            NativeFunction::from_closure(move |_this, args, _ctx| {
                use boa_engine::object::builtins::JsArrayBuffer;
                let name = args
                    .get_or_undefined(0)
                    .as_string()
                    .map(|s| s.to_std_string_escaped())
                    .unwrap_or_default();
                let bytes = args
                    .get_or_undefined(1)
                    .as_object()
                    .and_then(|o| JsArrayBuffer::from_object(o.clone()).ok())
                    .and_then(|ab| ab.to_vec())
                    .unwrap_or_default();
                log.borrow_mut().push((name, bytes));
                Ok(JsValue::undefined())
            })
        }
    };
    app.register_host_fn("saveFile", 2, save_file)
        .expect("register saveFile");

    save_log
}

// ---------------------------------------------------------------------------
// Tree-walk helpers (find text nodes / editable fields by content)
// ---------------------------------------------------------------------------

/// Absolute bounds (x, y, w, h) of the `tur_paragraph` whose `text` matches,
/// if any. Layout-independent click targeting.
fn find_text(app: &TurTestApp, target: &str) -> Option<(f64, f64, f64, f64)> {
    let root = app.dev_tool_element_tree()?;
    let mut stack: Vec<tur_engine::core::element::NodeId> = vec![root.id];
    while let Some(id) = stack.pop() {
        let node = app.dev_tool_get_element(id)?;
        if node.name == "tur_paragraph" {
            for (k, v) in &node.props {
                if *k == "text" {
                    if let TraceValue::Str(s) = v {
                        if s == target {
                            return Some((node.absolute.0, node.absolute.1, node.size.0, node.size.1));
                        }
                    }
                }
            }
        }
        stack.extend(node.children);
    }
    None
}

/// All `tur_editable_text` nodes, sorted top-to-bottom by y.
fn editables_by_y(app: &TurTestApp) -> Vec<(f64, f64, f64, f64)> {
    let Some(root) = app.dev_tool_element_tree() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut stack: Vec<tur_engine::core::element::NodeId> = vec![root.id];
    while let Some(id) = stack.pop() {
        if let Some(node) = app.dev_tool_get_element(id) {
            if node.name == "tur_editable_text" {
                out.push((node.absolute.0, node.absolute.1, node.size.0, node.size.1));
            }
            stack.extend(node.children);
        }
    }
    out.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
    out
}

fn type_into(app: &mut TurTestApp, x: f64, y: f64, text: &str) {
    app.click(x, y);
    for ch in text.chars() {
        app.send_key(&ch.to_string());
    }
}

// ---------------------------------------------------------------------------
// The test
// ---------------------------------------------------------------------------

#[test]
fn webdav_connect_browse_and_layout() {
    let mut app = TurTestApp::new(1000.0, 700.0).unwrap();
    let fs = Rc::new(RefCell::new(FakeFs::seed()));
    app.with_app_mut(|a| register_webdav_fakes(a, fs.clone()));
    app.load_bundle("webdav-client").unwrap();
    app.render();

    // Server list screen is showing. Open the connect dialog.
    let (x, y, w, h) = find_text(&app, "Add Server").expect("Add Server button visible");
    app.click(x + w / 2.0, y + h / 2.0);

    // The dialog has 4 fields top-to-bottom: Name, Server URL, Username, Password.
    let fields = editables_by_y(&app);
    assert!(fields.len() >= 4, "expected 4 dialog fields, got {}", fields.len());
    let url_f = fields[1];
    let user_f = fields[2];
    let pass_f = fields[3];
    type_into(&mut app, url_f.0 + 8.0, url_f.1 + 8.0, "http://webdav.test/dav/");
    type_into(&mut app, user_f.0 + 8.0, user_f.1 + 8.0, "world");
    type_into(&mut app, pass_f.0 + 8.0, pass_f.1 + 8.0, "a123456");

    // Save -> a server card appears in the list.
    let (sx, sy, sw, sh) = find_text(&app, "Save").expect("Save button");
    app.click(sx + sw / 2.0, sy + sh / 2.0);

    // Connect -> explorer opens and PROPFIND resolves (synchronously) to the
    // fake server's root listing.
    let (cx, cy, cw, ch) = find_text(&app, "Connect").expect("Connect button");
    app.click(cx + cw / 2.0, cy + ch / 2.0);
    app.render();

    // The PROPFIND must have parsed into the file list.
    let docs = find_text(&app, "Documents").expect("Documents entry listed");
    let welcome = find_text(&app, "welcome.txt").expect("welcome.txt entry listed");

    // Layout: the breadcrumb ("Root") sits at the top of the explorer, the
    // toolbar ("New Folder") directly below it, and the file rows fill the
    // area beneath. This is the assertion the original `Expanded`-in-`Row`
    // bug broke (everything collapsed to the bottom).
    let root = find_text(&app, "Root").expect("Root breadcrumb");
    let new_folder = find_text(&app, "New Folder").expect("New Folder toolbar button");

    assert!(
        root.1 < 120.0,
        "breadcrumb 'Root' should be near the top (y<120), got y={}",
        root.1
    );
    assert!(
        new_folder.1 > root.1 && new_folder.1 < 200.0,
        "toolbar 'New Folder' should be just below the top bar ({} < y < 200), got y={}",
        root.1,
        new_folder.1
    );
    assert!(
        docs.1 > new_folder.1,
        "file rows should be below the toolbar; Documents y={} <= toolbar y={}",
        docs.1,
        new_folder.1
    );
    assert!(
        welcome.1 > docs.1 || (welcome.1 - docs.1).abs() < 5.0,
        "welcome.txt should be at/after Documents"
    );
}
