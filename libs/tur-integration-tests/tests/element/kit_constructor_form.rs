use tur_engine::core::element::{ElementKind, ElementNodeId};
use tur_integration_tests::TurTestApp;

/// The `[constructor]` call form over the kit: the module below constructs
/// every element through `Type(..)` — the call form over each class's
/// designated `[constructor] fn builder` — never through the long form.
/// Arity-0 calls (`Column()`, `Row()`, `Text()`), parameter forwarding
/// (`SizedBox(400.0, 200.0)`), and the animation kit's `Opacity(0.5)` all
/// lower byte-identically to their long-form member calls.
const CTOR_FORM_RUT: &str = r#"

use tur_kit::handles::{ mount };
use tur_kit::layout::box::{ SizedBox };
use tur_kit::layout::flex::{ Column, Row };
use tur_kit::text::core::{ Text };
use tur_anim_kit::{ Opacity };

entry fn start() {
    let col = Column().query_key("ctor-col").child(Row().query_key("ctor-row").child(Text().text("x").query_key("ctor-text").build()).build()).child(SizedBox(400.0, 200.0).query_key("ctor-sized").build()).child(Opacity(0.5).child(Text().text("faded").build()).build());
    mount(col.build());
}
"#;

#[test]
fn kit_constructor_call_form_builds_the_tree() {
    let mut app = TurTestApp::new(800.0, 600.0).unwrap();
    app.load_rut_module(CTOR_FORM_RUT).unwrap();
    app.wait_for_timeout(std::time::Duration::ZERO);

    // Every query key resolves — the call forms compiled and mounted.
    for key in ["ctor-col", "ctor-row", "ctor-text", "ctor-sized"] {
        assert!(
            app.query_element(&[key]).is_some(),
            "queryKey '{key}' not found — the call form did not mount"
        );
    }

    let tree = app.element_tree();
    let col_id = app.query_element(&["ctor-col"]).unwrap();
    let col = tree
        .get_element(ElementNodeId::new(col_id.as_u64()))
        .unwrap();
    assert_eq!(col.kind().unwrap(), ElementKind::new("tur_flex"));
    assert_eq!(col.children.len(), 3, "Column() accumulated its children");

    // Row() + Text() inside it.
    let row = tree
        .get_element(ElementNodeId::new(col.children[0].as_u64()))
        .unwrap();
    assert_eq!(row.kind().unwrap(), ElementKind::new("tur_flex"));
    let text = tree
        .get_element(ElementNodeId::new(row.children[0].as_u64()))
        .unwrap();
    assert_eq!(text.kind().unwrap(), ElementKind::new("tur_paragraph"));

    // Parameter forwarding: SizedBox(400.0, 200.0) laid out at its args.
    let sized = tree
        .get_element(ElementNodeId::new(col.children[1].as_u64()))
        .unwrap();
    assert_eq!(sized.kind().unwrap(), ElementKind::new("tur_container"));
    assert_eq!(sized.computed_layout.size.width, 400.0);
    assert_eq!(sized.computed_layout.size.height, 200.0);

    // Opacity(0.5) from the animation kit wraps its child.
    let opacity = tree
        .get_element(ElementNodeId::new(col.children[2].as_u64()))
        .unwrap();
    assert_eq!(opacity.kind().unwrap(), ElementKind::new("tur_opacity"));
    let faded = tree
        .get_element(ElementNodeId::new(opacity.children[0].as_u64()))
        .unwrap();
    assert_eq!(faded.kind().unwrap(), ElementKind::new("tur_paragraph"));
}
