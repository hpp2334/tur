//! Host-registered image resources: the embedder creates the resource
//! host-side and hands JS only the handle — pixel bytes never enter the JS
//! realm.
//!
//! - `TurApp::register_image(image)` mints a **host-range** id (`>=
//!   HOST_IMAGE_ID_BASE`, counting down so every id stays f64-exact),
//!   retains the pixel Blob on the host + uploads it to the renderer
//!   directly, and notifies the worker with just the natural size (FIFO —
//!   the size is recorded before any rail can expose the id to JS).
//! - JS wraps the delivered numeric id via `imageResourceHandle(id)` (a
//!   validated, JS-opaque `ImageResourceHandle`); `Image().resourceId(...)`
//!   accepts the handle (and, back-compat, a plain number).

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use tur_engine::core::element::{ElementKind, ElementNodeId};
use tur_engine::core::elements::NodeTreeSnapshot;
use tur_engine::core::image_resource::{HOST_IMAGE_ID_BASE, ImageResource, ImageResourceId};
use tur_engine::core::render::{RenderCommand, Renderer};
use tur_integration_tests::TurTestApp;

/// Records `render_commands` + `upload_image_resource` calls into a shared
/// log (call order preserved — the replay-must-precede-first-frame assertion
/// reads it).
struct RecordingRenderer {
    calls: Rc<RefCell<Vec<String>>>,
}

impl Renderer for RecordingRenderer {
    fn render_commands(&mut self, commands: &[RenderCommand]) {
        self.calls
            .borrow_mut()
            .push(format!("render_commands:{}", commands.len()));
    }
    /// Idempotent, like the real renderers (the `contains_key` guard in
    /// `VelloRenderer`/`WebGlVelloRenderer`) — a repeated ensure for an
    /// already-present id is logged once.
    fn upload_image_resource(&mut self, id: ImageResourceId, _image: &ImageResource) {
        let mut calls = self.calls.borrow_mut();
        let entry = format!("upload:{}", id.as_u64());
        if !calls.contains(&entry) {
            calls.push(entry);
        }
    }
}

/// The 1×1 PNG used by the JS-decode path (`createImageResource`).
const PNG_1X1: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0,
    0, 0, 31, 21, 196, 137, 0, 0, 0, 13, 73, 68, 65, 84, 120, 218, 99, 252, 255, 159, 161, 30, 0,
    7, 130, 2, 127, 61, 200, 72, 239, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];

/// All `tur_image` nodes in the snapshot, in tree order, as `(width, height)`.
fn image_sizes(tree: &NodeTreeSnapshot) -> Vec<(f64, f64)> {
    let mut out = Vec::new();
    fn walk(tree: &NodeTreeSnapshot, id: ElementNodeId, out: &mut Vec<(f64, f64)>) {
        let node = tree.get_element(id).expect("snapshot node");
        if node.kind() == Some(ElementKind::new("tur_image")) {
            out.push((
                node.computed_layout.size.width,
                node.computed_layout.size.height,
            ));
        }
        for child in node.children.clone() {
            walk(tree, ElementNodeId::new(child.as_u64()), out);
        }
    }
    if let Some(root) = tree.root_element_id() {
        walk(tree, root, &mut out);
    }
    out
}

/// A host-registered image lays out at its natural size (the worker learned
/// the metadata from `RegisterImageMetadata`) and the pixel Blob is retained
/// host-side without any JS decode ever running.
#[test]
fn host_image_lays_out_at_natural_size() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();

    // 4×2 RGBA — registered entirely host-side; JS only ever sees the id.
    let rgba = vec![200u8; 4 * 2 * 4];
    let id = app
        .with_app(|a| a.register_image(ImageResource::from_rgba(&rgba, 4, 2).expect("rgba dims")));
    assert!(
        id.as_u64() >= HOST_IMAGE_ID_BASE,
        "host ids live in the disjoint host range"
    );

    // The root Column gives the image an unbounded main axis, so with no
    // explicit height the element sizes to the registered natural height (2).
    // The host-minted id crosses to the module through the entry rail.
    app.load_rut_module(
        r#"
use tur_host::{ BOXFIT_FILL, mount };
use tur_kit::{ Column, Image };


entry fn start() {
}

entry fn mount_host(host_id: u64, _b: f64) {
    let col = Column().child(Image(host_id).width(4.0).fit(BOXFIT_FILL).build());
    mount(col.build());
}
"#,
    )
    .expect("load module");
    app.call_rut_entry("mount_host", id.as_u64(), 0.0)
        .expect("mount host image");
    app.wait_for_timeout(Duration::ZERO);

    let tree = app.element_tree();
    let sizes = image_sizes(&tree);
    assert_eq!(sizes.len(), 1, "exactly one image mounted");
    assert_eq!(sizes[0].0, 4.0, "explicit width");
    assert_eq!(
        sizes[0].1, 2.0,
        "natural height from host-registered metadata (0 would mean the worker never learned it)"
    );

    // Host-side retention: the Blob never shipped from JS, yet main holds it.
    let count = app.with_app(|a| a.image_resource_count());
    assert_eq!(count, 1, "host retains the registered pixel Blob");
}

/// Worker-minted ids (JS `createImageResource`) and host-minted ids coexist
/// in one instance: disjoint ranges, one shared host-side map.
#[test]
fn host_and_js_image_ids_coexist() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();

    let rgba = vec![90u8; 4 * 2 * 4];
    let host_id = app
        .with_app(|a| a.register_image(ImageResource::from_rgba(&rgba, 4, 2).expect("rgba dims")));

    app.load_rut_module(
        r#"
use tur_host::{ BOXFIT_FILL, img_res_solid, mount };
use tur_kit::{ Column, Image };

entry fn start() {
}

// One worker-minted + one host-registered id, mounted side by side.
entry fn mount_both(host_id: u64, _b: f64) {
    let worker_id = img_res_solid(1, 1, 0xFF0000FFu64);
    let col = Column()
        .child(Image(worker_id).width(1.0).fit(BOXFIT_FILL).build())
        .child(Image(host_id).width(4.0).fit(BOXFIT_FILL).build());
    mount(col.build());
}
"#,
    )
    .expect("load module");
    app.call_rut_entry("mount_both", host_id.as_u64(), 0.0)
        .expect("mount both");
    app.wait_for_timeout(Duration::ZERO);

    assert!(
        !host_id.is_host_minted() || host_id.as_u64() >= HOST_IMAGE_ID_BASE,
        "host id must carry the host-range marker"
    );
    let count = app.with_app(|a| a.image_resource_count());
    assert_eq!(
        count, 2,
        "both resources retained host-side (1 worker-minted + 1 host-registered)"
    );

    let tree = app.element_tree();
    let sizes = image_sizes(&tree);
    assert_eq!(sizes.len(), 2, "both images mounted");
    assert_eq!(sizes[0].1, 1.0, "worker-minted 1×1 natural height");
    assert_eq!(sizes[1].1, 2.0, "host-registered 4×2 natural height");
}

/// Detach → attach: the freshly attached renderer paints every
/// previously-registered image. Both retention rails share one map (the
/// `UploadImage` arm for worker-decoded images + `register_image` for
/// host-registered ones), and the render commit point re-ensures every
/// image a frame references before painting it — a JS-cached handle only
/// ever fetches missing ids, so without the ensure-pass a re-attached
/// renderer (empty atlas) would render previously-registered images blank
/// forever. The ensures must land BEFORE the first frame paints.
#[test]
fn reattach_ensures_retained_images_before_first_frame() {
    let app = TurTestApp::new_with_renderer(
        400.0,
        600.0,
        Box::new(RecordingRenderer {
            calls: Rc::new(RefCell::new(Vec::new())),
        }),
    )
    .unwrap();

    // Both retention rails: one host-registered + one worker-minted image.
    let rgba = vec![7u8; 4 * 2 * 4];
    let host_id = app
        .with_app(|a| a.register_image(ImageResource::from_rgba(&rgba, 4, 2).expect("rgba dims")));

    app.load_rut_module(
        r#"use tur_host::{ BOXFIT_FILL, img_res_solid, mount };
use tur_kit::{ Column, Image };



entry fn start() {
}

entry fn mount_both(host_id: u64, _b: f64) {
    let worker_id = img_res_solid(1, 1, 0xFF0000FFu64);
    let col = Column()
        .child(Image(worker_id).width(1.0).fit(BOXFIT_FILL).build())
        .child(Image(host_id).width(4.0).fit(BOXFIT_FILL).build());
    mount(col.build());
}
"#,
    )
    .expect("load module");
    app.call_rut_entry("mount_both", host_id.as_u64(), 0.0)
        .expect("mount both");
    app.wait_for_timeout(Duration::ZERO);

    let count = app.with_app(|a| a.image_resource_count());
    assert_eq!(count, 2, "both resources retained host-side");

    // DETACH → ATTACH a fresh renderer at a different size (the changed
    // viewport forces a relayout + repaint, so the fresh log carries frames
    // to order the ensures against).
    app.with_app(|a| a.detach_renderer());
    let fresh_calls = Rc::new(RefCell::new(Vec::new()));
    app.with_app(|a| {
        a.attach_renderer(
            Box::new(RecordingRenderer {
                calls: fresh_calls.clone(),
            }),
            320,
            480,
            1.0,
        )
    });
    app.wait_for_timeout(Duration::ZERO);

    let log = fresh_calls.borrow();
    let first_frame = log
        .iter()
        .position(|c| c.starts_with("render_commands:"))
        .expect("fresh renderer must paint after attach");
    let upload_positions: Vec<usize> = log
        .iter()
        .enumerate()
        .filter(|(_, c)| c.starts_with("upload:"))
        .map(|(i, _)| i)
        .collect();
    assert_eq!(
        upload_positions.len(),
        2,
        "both mounted images must be ensured into the fresh renderer, got {log:?}"
    );
    assert!(
        upload_positions
            .iter()
            .any(|&i| log[i] == format!("upload:{}", host_id.as_u64())),
        "host-registered id must be among the ensured resources, got {log:?}"
    );
    assert!(
        upload_positions.iter().all(|&i| i < first_frame),
        "ensures must land before the first frame paints on the fresh renderer, got {log:?}"
    );
}

/// Pay for what you paint: the render commit point re-ensures only the
/// images a frame actually references. Three resources are retained but the
/// module paints one — the freshly attached renderer must be ensured with
/// exactly that one (the eager full-map replay form uploaded all three at
/// attach, charging surface recreation for images the frame never shows).
#[test]
fn reattach_uploads_only_painted_images() {
    let app = TurTestApp::new_with_renderer(
        400.0,
        600.0,
        Box::new(RecordingRenderer {
            calls: Rc::new(RefCell::new(Vec::new())),
        }),
    )
    .unwrap();

    // Three retained resources; the module paints only the first.
    let ids: Vec<ImageResourceId> = (0..3)
        .map(|i| {
            let rgba = vec![10u8 + i; 4 * 2 * 4];
            app.with_app(|a| {
                a.register_image(ImageResource::from_rgba(&rgba, 4, 2).expect("rgba dims"))
            })
        })
        .collect();

    app.load_rut_module(
        r#"use tur_host::{ BOXFIT_FILL, mount };
use tur_kit::{ Image };

use tur_kit::{ Image };


entry fn start() {
}

entry fn mount_one(id: u64, _b: f64) {
    mount(Image(id).width(4.0).fit(BOXFIT_FILL).build());
}
"#,
    )
    .expect("load module");
    app.call_rut_entry("mount_one", ids[0].as_u64(), 0.0)
        .expect("mount one");
    app.wait_for_timeout(Duration::ZERO);

    let count = app.with_app(|a| a.image_resource_count());
    assert_eq!(count, 3, "all three resources retained host-side");

    // DETACH → ATTACH: only the painted image may be ensured.
    app.with_app(|a| a.detach_renderer());
    let fresh_calls = Rc::new(RefCell::new(Vec::new()));
    app.with_app(|a| {
        a.attach_renderer(
            Box::new(RecordingRenderer {
                calls: fresh_calls.clone(),
            }),
            320,
            480,
            1.0,
        )
    });
    app.wait_for_timeout(Duration::ZERO);

    let log = fresh_calls.borrow();
    let uploads: Vec<&String> = log.iter().filter(|c| c.starts_with("upload:")).collect();
    assert_eq!(
        uploads,
        vec![&format!("upload:{}", ids[0].as_u64())],
        "only the painted image may be ensured on the fresh renderer, got {log:?}"
    );
}

/// The rut twin of the boa `createSvgResource`: an SVG string authored in
/// the module rasterises worker-side and registers through the same rail
/// as `img_res_bytes` — the id mints in the worker range, the declared
/// size crosses the metadata rail, and the pixel Blob is retained
/// host-side. (The playground toolbar's ▶ / ↻ icons ride this row.)
#[test]
fn img_res_svg_row_registers_a_worker_minted_resource() {
    let app = TurTestApp::new(400.0, 600.0).unwrap();
    app.load_rut_module(
        r##"
use tur_host::{ BOXFIT_FILL, img_res_svg, mount };
use tur_kit::{ Column, Image };

entry fn start() {
}

entry fn mount_svg(_a: u64, _b: f64) {
    let id = img_res_svg("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"24\" height=\"24\" viewBox=\"0 0 24 24\" fill=\"#ffffff\"><polygon points=\"6 4 20 12 6 20\"/></svg>");
    mount(Column().child(Image(id).width(10.0).fit(BOXFIT_FILL).build()).build());
}
"##,
    )
    .expect("load module");
    app.call_rut_entry("mount_svg", 0, 0.0).expect("mount svg");
    app.wait_for_timeout(Duration::ZERO);

    let tree = app.element_tree();
    let sizes = image_sizes(&tree);
    assert_eq!(sizes.len(), 1, "exactly one image mounted");
    assert_eq!(sizes[0].0, 10.0, "explicit width");
    assert_eq!(
        sizes[0].1, 24.0,
        "natural height from the SVG's declared size (the decode + metadata rail)"
    );
    let count = app.with_app(|a| a.image_resource_count());
    assert_eq!(count, 1, "the rasterised resource is retained host-side");
}
