//! The 8 enum const-objects exported by `tur:std` (Axis,
//! MainAxisAlignment, …). Each mirrors a `core::layout` C-like enum so JS
//! callers write `Axis.Vertical` and `Axis[0] === "Vertical"`.

use boa_engine::js_string;
use boa_engine::object::JsObject;
use boa_engine::{Context, JsValue};

use crate::core::js_runtime::helpers::ConstEntry;

/// Build a JS object modelling a TypeScript numeric `enum`: forward mapping
/// (`Vertical: 0`) AND reverse mapping (`"0": "Vertical"`), exactly as `tsc`
/// emits for `enum Axis { Vertical, Horizontal }`.
fn build_enum(context: &mut Context, pairs: &[(&str, u32)]) -> JsValue {
    let obj = JsObject::with_object_proto(context.intrinsics());
    for (name, val) in pairs {
        // forward: name -> number
        let _ = obj.create_data_property(
            js_string!(*name),
            JsValue::from(*val as f64),
            context,
        );
        // reverse: number (as string key) -> name
        let _ = obj.create_data_property(
            js_string!(val.to_string()),
            JsValue::from(js_string!(*name)),
            context,
        );
    }
    obj.into()
}

/// All enum const-objects, ready to merge into the module's const exports.
pub fn consts(context: &mut Context) -> Vec<ConstEntry> {
    vec![
        ("Axis", build_enum(context, &[("Vertical", 0), ("Horizontal", 1)])),
        (
            "MainAxisAlignment",
            build_enum(
                context,
                &[
                    ("Start", 0),
                    ("Center", 1),
                    ("End", 2),
                    ("SpaceBetween", 3),
                    ("SpaceAround", 4),
                    ("SpaceEvenly", 5),
                ],
            ),
        ),
        (
            "CrossAxisAlignment",
            build_enum(context, &[("Start", 0), ("Center", 1), ("End", 2), ("Stretch", 3)]),
        ),
        ("MainAxisSize", build_enum(context, &[("Max", 0), ("Min", 1)])),
        (
            "HitTestBehavior",
            build_enum(context, &[("Opaque", 0), ("Translucent", 1)]),
        ),
        (
            "BoxFit",
            build_enum(
                context,
                &[
                    ("Fill", 0),
                    ("Contain", 1),
                    ("Cover", 2),
                    ("FitWidth", 3),
                    ("FitHeight", 4),
                    ("None", 5),
                ],
            ),
        ),
        (
            "BorderPosition",
            build_enum(context, &[("Inside", 0), ("Center", 1), ("Outside", 2)]),
        ),
        (
            "Alignment",
            build_enum(
                context,
                &[
                    ("TopLeft", 0),
                    ("TopCenter", 1),
                    ("TopRight", 2),
                    ("CenterLeft", 3),
                    ("Center", 4),
                    ("CenterRight", 5),
                    ("BottomLeft", 6),
                    ("BottomCenter", 7),
                    ("BottomRight", 8),
                ],
            ),
        ),
    ]
}
