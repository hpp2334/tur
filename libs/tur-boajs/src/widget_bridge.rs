use std::sync::LazyLock;
use std::sync::RwLock;

use boa_engine::js_string;
use boa_engine::native_function::NativeFunction;
use boa_engine::object::{FunctionObjectBuilder, ObjectInitializer};
use boa_engine::property::Attribute;
use boa_engine::{Context, JsObject, JsResult, JsValue};
use tracing;
use tur_widget::{PropValue, WidgetKind, WidgetNode, WidgetTree};

static WIDGET_TREE: LazyLock<RwLock<WidgetTree>> = LazyLock::new(|| RwLock::new(WidgetTree::new()));
static NEXT_ID: LazyLock<RwLock<u64>> = LazyLock::new(|| RwLock::new(1));

fn alloc_id() -> u64 {
    let mut next = NEXT_ID.write().unwrap();
    let id = *next;
    *next += 1;
    id
}

pub fn widget_tree() -> &'static LazyLock<RwLock<WidgetTree>> {
    &WIDGET_TREE
}

pub fn create_widget_namespace(context: &mut Context) -> JsObject {
    let realm = context.realm();

    let create_element_fn =
        FunctionObjectBuilder::new(realm, NativeFunction::from_fn_ptr(js_create_element))
            .length(1)
            .build();

    let set_attribute_fn =
        FunctionObjectBuilder::new(realm, NativeFunction::from_fn_ptr(js_set_attribute))
            .length(3)
            .build();

    let append_child_fn =
        FunctionObjectBuilder::new(realm, NativeFunction::from_fn_ptr(js_append_child))
            .length(2)
            .build();

    let remove_child_fn =
        FunctionObjectBuilder::new(realm, NativeFunction::from_fn_ptr(js_remove_child))
            .length(2)
            .build();

    let insert_before_fn =
        FunctionObjectBuilder::new(realm, NativeFunction::from_fn_ptr(js_insert_before))
            .length(3)
            .build();

    let get_parent_fn =
        FunctionObjectBuilder::new(realm, NativeFunction::from_fn_ptr(js_get_parent))
            .length(1)
            .build();

    let get_first_child_fn =
        FunctionObjectBuilder::new(realm, NativeFunction::from_fn_ptr(js_get_first_child))
            .length(1)
            .build();

    let get_next_sibling_fn =
        FunctionObjectBuilder::new(realm, NativeFunction::from_fn_ptr(js_get_next_sibling))
            .length(1)
            .build();

    ObjectInitializer::new(context)
        .property(
            js_string!("createElement"),
            create_element_fn,
            Attribute::all(),
        )
        .property(
            js_string!("setAttribute"),
            set_attribute_fn,
            Attribute::all(),
        )
        .property(js_string!("appendChild"), append_child_fn, Attribute::all())
        .property(js_string!("removeChild"), remove_child_fn, Attribute::all())
        .property(
            js_string!("insertBefore"),
            insert_before_fn,
            Attribute::all(),
        )
        .property(js_string!("getParent"), get_parent_fn, Attribute::all())
        .property(
            js_string!("getFirstChild"),
            get_first_child_fn,
            Attribute::all(),
        )
        .property(
            js_string!("getNextSibling"),
            get_next_sibling_fn,
            Attribute::all(),
        )
        .build()
}

fn js_create_element(
    _this: &JsValue,
    args: &[JsValue],
    _context: &mut Context,
) -> JsResult<JsValue> {
    let kind_str = args
        .first()
        .and_then(|v| v.as_string())
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    let kind = WidgetKind::from_str(&kind_str).unwrap_or_else(|| {
        tracing::warn!("unknown widget type: {kind_str}, falling back to Container");
        WidgetKind::Container
    });

    let id = alloc_id();
    let node = WidgetNode::new(id, kind);

    let mut tree = WIDGET_TREE.write().unwrap();
    tree.insert(node);

    tracing::trace!("createElement({kind_str}) -> {id}");
    Ok(JsValue::from(id as f64))
}

fn js_set_attribute(_this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let id = args.first().and_then(|v| v.as_number()).unwrap_or(0.0) as u64;
    let key = args
        .get(1)
        .and_then(|v| v.as_string())
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();
    let value = args.get(2).cloned().unwrap_or(JsValue::Undefined);

    let prop_value = match &value {
        JsValue::String(s) => PropValue::String(s.to_std_string_escaped()),
        JsValue::Rational(n) => PropValue::Number(*n),
        JsValue::Integer(n) => PropValue::Number(*n as f64),
        JsValue::Boolean(b) => PropValue::Bool(*b),
        JsValue::BigInt(b) => {
            let n: i64 = b.to_string().parse().unwrap_or(0);
            PropValue::Number(n as f64)
        }
        _ => PropValue::String(
            value
                .to_string(context)
                .map(|s| s.to_std_string_escaped())
                .unwrap_or_default(),
        ),
    };

    tracing::trace!("setAttribute({id}, {key}, ...)");

    let mut tree = WIDGET_TREE.write().unwrap();
    if let Some(node) = tree.get_mut(id) {
        node.set_prop(key, prop_value);
    }

    Ok(JsValue::Undefined)
}

fn js_append_child(_this: &JsValue, args: &[JsValue], _context: &mut Context) -> JsResult<JsValue> {
    let parent_id = args.first().and_then(|v| v.as_number()).unwrap_or(0.0) as u64;
    let child_id = args.get(1).and_then(|v| v.as_number()).unwrap_or(0.0) as u64;

    let mut tree = WIDGET_TREE.write().unwrap();
    tree.append_child(parent_id, child_id);

    tracing::trace!("appendChild({parent_id}, {child_id})");
    Ok(JsValue::Undefined)
}

fn js_remove_child(_this: &JsValue, args: &[JsValue], _context: &mut Context) -> JsResult<JsValue> {
    let parent_id = args.first().and_then(|v| v.as_number()).unwrap_or(0.0) as u64;
    let child_id = args.get(1).and_then(|v| v.as_number()).unwrap_or(0.0) as u64;

    let mut tree = WIDGET_TREE.write().unwrap();
    tree.remove_child(parent_id, child_id);

    tracing::trace!("removeChild({parent_id}, {child_id})");
    Ok(JsValue::Undefined)
}

fn js_insert_before(
    _this: &JsValue,
    args: &[JsValue],
    _context: &mut Context,
) -> JsResult<JsValue> {
    let parent_id = args.first().and_then(|v| v.as_number()).unwrap_or(0.0) as u64;
    let child_id = args.get(1).and_then(|v| v.as_number()).unwrap_or(0.0) as u64;
    let ref_id = args.get(2).and_then(|v| v.as_number()).unwrap_or(0.0) as u64;

    let mut tree = WIDGET_TREE.write().unwrap();
    tree.insert_before(parent_id, child_id, ref_id);

    tracing::trace!("insertBefore({parent_id}, {child_id}, {ref_id})");
    Ok(JsValue::Undefined)
}

fn js_get_parent(_this: &JsValue, args: &[JsValue], _context: &mut Context) -> JsResult<JsValue> {
    let id = args.first().and_then(|v| v.as_number()).unwrap_or(0.0) as u64;
    let tree = WIDGET_TREE.read().unwrap();
    match tree.parent_of(id) {
        Some(parent_id) => Ok(JsValue::from(parent_id as f64)),
        None => Ok(JsValue::Null),
    }
}

fn js_get_first_child(
    _this: &JsValue,
    args: &[JsValue],
    _context: &mut Context,
) -> JsResult<JsValue> {
    let id = args.first().and_then(|v| v.as_number()).unwrap_or(0.0) as u64;
    let tree = WIDGET_TREE.read().unwrap();
    match tree.first_child_of(id) {
        Some(child_id) => Ok(JsValue::from(child_id as f64)),
        None => Ok(JsValue::Null),
    }
}

fn js_get_next_sibling(
    _this: &JsValue,
    args: &[JsValue],
    _context: &mut Context,
) -> JsResult<JsValue> {
    let id = args.first().and_then(|v| v.as_number()).unwrap_or(0.0) as u64;
    let tree = WIDGET_TREE.read().unwrap();
    match tree.next_sibling_of(id) {
        Some(sibling_id) => Ok(JsValue::from(sibling_id as f64)),
        None => Ok(JsValue::Null),
    }
}
