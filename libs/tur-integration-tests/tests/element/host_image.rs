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

use std::time::Duration;

use tur_engine::EventBus;
use tur_engine::core::element::{ElementKind, ElementNodeId};
use tur_engine::core::elements::NodeTreeSnapshot;
use tur_engine::core::image_resource::{HOST_IMAGE_ID_BASE, ImageResource};
use tur_integration_tests::TurTestApp;

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
    app.eval_module_source(&format!(
        r#"
        import {{ Column, imageResourceHandle, Image, mount, view }} from "tur:std";
        const handle = imageResourceHandle({});
        export function start() {{
            mount(view(() => Column().children([
                Image().resourceId(handle).width(4).build(),
            ]).build()));
        }}
        "#,
        id.as_u64()
    ))
    .expect("load module");
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

    app.eval_module_source(&format!(
        r#"
        import {{ Column, createImageResource, imageResourceHandle, Image, mount, view }} from "tur:std";
        const pngBytes = new Uint8Array({png:?});
        const jsHandle = createImageResource(pngBytes);   // worker-minted id
        const hostHandle = imageResourceHandle({host_id}); // host-minted id
        export function start() {{
            mount(view(() => Column().children([
                Image().resourceId(jsHandle).width(1).build(),
                Image().resourceId(hostHandle).width(4).build(),
            ]).build()));
        }}
        "#,
        png = PNG_1X1,
        host_id = host_id.as_u64(),
    ))
    .expect("load module");
    app.wait_for_timeout(Duration::ZERO);

    assert!(
        !host_id.is_host_minted() || host_id.as_u64() >= HOST_IMAGE_ID_BASE,
        "host id must carry the host-range marker"
    );
    let count = app.with_app(|a| a.image_resource_count());
    assert_eq!(
        count, 2,
        "both resources retained host-side (1 JS-shipped + 1 host-registered)"
    );

    let tree = app.element_tree();
    let sizes = image_sizes(&tree);
    assert_eq!(sizes.len(), 2, "both images mounted");
    assert_eq!(sizes[0].1, 1.0, "JS-decoded 1×1 natural height");
    assert_eq!(sizes[1].1, 2.0, "host-registered 4×2 natural height");
}

/// The delivery rail: the host emits the id over the event bus (f64 bits),
/// JS decodes + wraps it — `imageResourceHandle` validating against the
/// worker's metadata proves the FIFO guarantee (metadata registered before
/// the id can reach JS). A module loaded afterwards mounts it.
#[test]
fn host_image_handle_via_event_bus() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();

    let rgba = vec![10u8; 3 * 5 * 4];
    let host_id = app
        .with_app(|a| a.register_image(ImageResource::from_rgba(&rgba, 3, 5).expect("rgba dims")));

    app.eval_module_source(
        r#"
        import { eventBus, imageResourceHandle } from "tur:std";
        globalThis.__wrapped = false;
        eventBus.on(7, (payload) => {
            const id = new DataView(payload.buffer, payload.byteOffset, payload.byteLength)
                .getFloat64(0, true);
            globalThis.__hostHandle = imageResourceHandle(id); // throws if the id is unknown
            globalThis.__wrapped = true;
        });
        "#,
    )
    .expect("load module");

    let bus = EventBus::of(app.app()).expect("event bus");
    bus.emit_to_js(7, (host_id.as_u64() as f64).to_le_bytes().to_vec());
    app.wait_for(|a| a.eval_js("globalThis.__wrapped") == "true");

    // Mount it in a fresh module: the handle survives the reload (same realm)
    // and drives a natural-sized image.
    app.eval_module_source(
        r#"
        import { Column, Image, mount, view } from "tur:std";
        const handle = globalThis.__hostHandle;
        export function start() {
            mount(view(() => Column().children([
                Image().resourceId(handle).width(3).build(),
            ]).build()));
        }
        "#,
    )
    .expect("load module");
    app.wait_for_timeout(Duration::ZERO);

    let tree = app.element_tree();
    let sizes = image_sizes(&tree);
    assert_eq!(sizes.len(), 1, "bus-delivered handle mounted");
    assert_eq!(
        sizes[0].1, 5.0,
        "natural height via the host-registered metadata"
    );
}

/// `imageResourceHandle` validates: an id the worker never heard of is a
/// loud module-load error, not a silent zero-sized image.
#[test]
fn image_resource_handle_rejects_unknown_id() {
    let mut app = TurTestApp::new(400.0, 600.0).unwrap();

    let err = app
        .eval_module_source(
            r#"
            import { imageResourceHandle } from "tur:std";
            const handle = imageResourceHandle(424242);
            "#,
        )
        .expect_err("unknown id must fail the load");
    assert!(
        err.to_string().contains("unknown image resource"),
        "error should name the problem, got: {err}"
    );
}
