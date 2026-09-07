//! The element **builder** — the JS construction surface for every view.
//!
//! `Container(props)` (and every sibling element constructor) returns a
//! *builder*: a JS object whose chainable methods accumulate props into a
//! plain JS object, terminated by `.build()`, which invokes the element's
//! terminal bridge fn (the classic `require_props_object` → `from_js` →
//! `wrap_view` path) with the accumulated object:
//!
//! ```js
//! Container()
//!     .padding(16)
//!     .color(bg)
//!     .children([Text({ text: "hi" }).fontSize(14).build()])
//!     .build()
//! ```
//!
//! Design constraints honored here (the engine's bridge discipline):
//!
//! - **Plain fn pointers only.** Every method is a distinct fn (one per
//!   prop key, generated once centrally below) whose prop key is baked into
//!   the fn body — no `NativeFunction` closures anywhere. Per-object state
//!   rides the builder object's [`BuilderState`] JsData payload, read off
//!   `this` — the same pattern as `Task.cancel` / `store.get`.
//! - **One shared prototype per element per realm.** Methods are attached
//!   to a prototype object cached on the realm's global (a hidden,
//!   non-enumerable, non-writable, non-configurable property keyed by the
//!   static table's address), so a builder instance costs one `JsObject` +
//!   its state — not a function object per method per construction. The
//!   global is a GC root, so the cached prototype is properly traced; it
//!   is NOT stored in `TurInstanceContext` (which is
//!   `unsafe_empty_trace` and must stay free of GC handles).
//! - **Terminals unchanged.** The terminal fns are exactly the historical
//!   bridge bodies (`extract_js_ctx` → `require_props_object(args, 1)` →
//!   `XxxView::from_js` → `wrap_view`); required-prop validation therefore
//!   surfaces at `.build()` time, identically to the old call shape.

use boa_engine::native_function::NativeFunction;
use boa_engine::object::FunctionObjectBuilder;
use boa_engine::object::JsObject;
use boa_engine::object::builtins::JsArray;
use boa_engine::property::PropertyDescriptor;
use boa_engine::{Context, JsArgs, JsResult, JsString, JsValue, js_string};
use boa_gc::{Finalize, Trace};

use crate::core::js_runtime::helpers::Ptr;

/// A chainable builder method: exposed JS method `name` writing prop `key`
/// via the plain fn pointer `ptr`.
#[derive(Clone, Copy)]
pub struct BuilderMethod {
    /// The JS method name builders call (`.padding(...)`).
    pub name: &'static str,
    /// The props-object key the method writes (usually identical to
    /// `name`; differs for collision renames like `itemBuilder` → `build`).
    pub key: &'static str,
    pub ptr: Ptr,
}

impl BuilderMethod {
    /// A method whose name and prop key are the same string.
    pub const fn new(name: &'static str, ptr: Ptr) -> Self {
        BuilderMethod {
            name,
            key: name,
            ptr,
        }
    }

    /// A method whose JS name differs from the prop key it writes.
    pub const fn renamed(name: &'static str, key: &'static str, ptr: Ptr) -> Self {
        BuilderMethod { name, key, ptr }
    }
}

/// The per-element builder surface: the prop methods plus which generic
/// child methods are exposed.
pub struct BuilderTable {
    pub methods: &'static [BuilderMethod],
    /// Expose `.child(el)` — sets the `child` prop (single-child elements
    /// and thunk-children like `Condition.child`). Also accepts the plain
    /// `child` setter when an element declares it explicitly.
    pub child: bool,
    /// Expose `.children([...])` — appends into the `children` array prop.
    pub children: bool,
}

/// State riding the builder object's `JsData` payload, read off `this` by
/// every method (clone-out-before-JS discipline).
///
/// Sound under `unsafe_empty_trace` exactly like `TaskCancelState`: the
/// `JsValue`/`JsObject` clones held here are GC roots (boa roots `Gc`
/// handles on clone), so the collector never frees them while the payload
/// holds them; the fn pointer is pure Copy.
#[derive(Trace, Finalize, boa_engine::JsData)]
#[boa_gc(unsafe_empty_trace)]
pub struct BuilderState {
    /// The bridge ctx value (the `TurInstanceContext` opaque object),
    /// re-supplied as `args[0]` when `.build()` invokes the terminal.
    pub ctx: JsValue,
    /// The terminal bridge fn (ctx-first signature: `(this, [ctx, props])`).
    pub ctor: Ptr,
    /// The accumulating props object (starts as the constructor argument).
    pub props: JsObject,
}

/// The "call it as a method" TypeError every state read raises when `this`
/// isn't a builder.
fn not_a_builder() -> boa_engine::JsError {
    boa_engine::JsError::from(
        boa_engine::JsNativeError::typ()
            .with_message("expected the builder object as `this` — call it as a method (b.x())"),
    )
}

/// Clone the builder's accumulating props object off `this`.
fn builder_props(this: &JsValue) -> JsResult<JsObject> {
    let obj = this.as_object().ok_or_else(not_a_builder)?;
    let state = obj
        .downcast_ref::<BuilderState>()
        .ok_or_else(not_a_builder)?;
    Ok(state.props.clone())
}

/// Clone the terminal + ctx + props triple off `this` (for `.build()`).
fn builder_parts(this: &JsValue) -> JsResult<(Ptr, JsValue, JsObject)> {
    let obj = this.as_object().ok_or_else(not_a_builder)?;
    let state = obj
        .downcast_ref::<BuilderState>()
        .ok_or_else(not_a_builder)?;
    Ok((state.ctor, state.ctx.clone(), state.props.clone()))
}

/// Shared body of every generated setter: write `props[key] = args[0]`,
/// return `this` for chaining.
fn builder_set_prop(
    this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
    key: &str,
) -> JsResult<JsValue> {
    let props = builder_props(this)?;
    let value = args.get_or_undefined(0).clone();
    props.create_data_property_or_throw(js_string!(key), value, context)?;
    Ok(this.clone())
}

/// `.children(items)` — append semantics: extends the existing `children`
/// array (creating it on first call). Accepts an array of children or a
/// single child.
fn tur_builder_children(
    this: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let props = builder_props(this)?;
    let incoming = args.get_or_undefined(0).clone();

    // Existing array, or a fresh one on first call.
    let existing = props.get(js_string!("children"), context)?;
    let target = match existing
        .as_object()
        .and_then(|o| JsArray::from_object(o).ok())
    {
        Some(arr) => arr,
        None => JsArray::new(context)?,
    };

    if let Some(items) = incoming
        .as_object()
        .and_then(|o| JsArray::from_object(o).ok())
    {
        let len = items.length(context).unwrap_or(0);
        for i in 0..len {
            if let Ok(item) = items.at(i as i64, context) {
                target.push(item, context)?;
            }
        }
    } else if !incoming.is_null() && !incoming.is_undefined() {
        target.push(incoming, context)?;
    }

    props.create_data_property_or_throw(js_string!("children"), JsValue::from(target), context)?;
    Ok(this.clone())
}

/// `.child(el)` — sets the `child` prop (last write wins). The value may be
/// an element or a `() => Element` thunk (e.g. `Condition.child`).
fn tur_builder_child(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    builder_set_prop(this, args, context, "child")
}

/// `.build()` — the terminal. Invokes the element's terminal bridge fn with
/// the accumulated props object; returns the built `ViewHandle` (Element).
/// Required-prop validation fires here (inside the terminal's `from_js`).
/// Calling `build()` twice is allowed — specs are immutable and cheap.
fn tur_builder_build(
    this: &JsValue,
    _args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let (ctor, ctx_value, props) = builder_parts(this)?;
    (ctor)(&JsValue::undefined(), &[ctx_value, props.into()], context)
}

/// The shared prototype for one builder table: all prop methods plus the
/// generic `child`/`children` (per table flags) and `build`. Cached on the
/// realm's global object under a hidden property keyed by the table's
/// static address, so it is built once per realm and properly traced (the
/// global is a GC root).
fn builder_proto(table: &'static BuilderTable, context: &mut Context) -> JsObject {
    let key = JsString::from(format!("__turBuilderProto_{:p}", table));
    let global = context.global_object();

    if let Ok(v) = global.get(key.clone(), context)
        && let Some(proto) = v.as_object()
    {
        return proto;
    }

    let proto =
        JsObject::from_proto_and_data(context.intrinsics().constructors().object().prototype(), ());

    let define = |name: &str, ptr: Ptr, context: &mut Context| -> JsResult<()> {
        let f = FunctionObjectBuilder::new(context.realm(), NativeFunction::from_fn_ptr(ptr))
            .length(1)
            .name(js_string!(name))
            .build();
        proto.create_data_property_or_throw(js_string!(name), JsValue::from(f), context)?;
        Ok(())
    };

    for m in table.methods {
        let _ = define(m.name, m.ptr, context);
    }
    if table.child {
        let _ = define("child", tur_builder_child, context);
    }
    if table.children {
        let _ = define("children", tur_builder_children, context);
    }
    let _ = define("build", tur_builder_build, context);

    // Hidden + immutable: JS cannot enumerate, overwrite, or delete the
    // cache slot (not a security boundary — just hygiene).
    let _ = global.define_property_or_throw(
        key,
        PropertyDescriptor::builder()
            .value(proto.clone())
            .enumerable(false)
            .writable(false)
            .configurable(false)
            .build(),
        context,
    );

    proto
}

/// Construct a builder: an object with the table's shared prototype and a
/// fresh [`BuilderState`] (the constructor's props object as accumulator).
pub fn make_builder(
    ctx_value: JsValue,
    ctor: Ptr,
    table: &'static BuilderTable,
    props: JsObject,
    context: &mut Context,
) -> JsObject {
    let proto = builder_proto(table, context);
    JsObject::from_proto_and_data(
        proto,
        BuilderState {
            ctx: ctx_value,
            ctor,
            props,
        },
    )
}

/// Generate a builder-factory fn (the exported `FnEntry` target): validates
/// the constructor props object, then returns a builder for `terminal` per
/// `table`. Place next to the unchanged terminal fn in each bridge file.
///
/// ```ignore
/// builder_factory!(tur_container_factory, tur_container, &TABLE);
/// ```
macro_rules! builder_factory {
    ($factory:ident, $terminal:ident, $table:expr) => {
        pub(super) fn $factory(
            _this: &JsValue,
            args: &[JsValue],
            context: &mut Context,
        ) -> JsResult<JsValue> {
            let ctx_value = args.get(0).cloned().unwrap_or_default();
            let props = $crate::core::js_runtime::helpers::require_props_object(args, 1, context)?;
            Ok($crate::core::js_runtime::builder::make_builder(
                ctx_value, $terminal, $table, props, context,
            )
            .into())
        }
    };
}
pub(crate) use builder_factory;

/// The setter fns — one plain fn pointer per prop key, shared by every
/// element table that exposes the key. CamelCase fn names match the JS
/// method names verbatim (the codebase's bridge naming convention).
pub mod setters {
    use super::builder_set_prop;
    use boa_engine::{Context, JsResult, JsValue};

    macro_rules! builder_setters {
        ($($key:ident),* $(,)?) => {
            $(
                #[allow(non_snake_case)]
                pub fn $key(
                    this: &JsValue,
                    args: &[JsValue],
                    context: &mut Context,
                ) -> JsResult<JsValue> {
                    builder_set_prop(this, args, context, stringify!($key))
                }
            )*
        };
    }

    builder_setters! {
        // Container / sizing / decoration
        width, height, padding, color, borderColor, borderWidth, borderRadius,
        borderPosition, clipBehavior, shadowColor, shadowBlur, shadowOffset, alignment,
        // Flex + flex items
        mainAlignment, crossAlignment, mainAxisSize, flex,
        // Grid / table geometry
        maxCrossAxisExtent, childAspectRatio, mainAxisExtent, crossAxisSpacing,
        mainAxisSpacing, columns, rows, headerExtent, rowExtent, rowSpacing, stripeColor,
        dividerColor, dividerThickness,
        // Stack / positioned
        fit, left, top, right, bottom,
        // Text
        text, fontSize, fontWeight, spans, maxLines, overflow, selectable,
        onSelectionChange, fontFamily, multiline,
        // Input
        controller, undoController, placeholder, placeholderColor, cursorColor,
        obscureText, obscuringCharacter, onContextMenu,
        // Image
        resourceId,
        // Scroll
        axis, trackColor, thumbRadius, thickness,
        // Lazy containers
        itemCount, builder, overscan, itemExtent,
        // Effects
        value, scale, scaleX, scaleY, rotate, translateX, translateY,
        // Composited transform
        link, targetOffset, showWhenUnlinked, targetAnchor, followerAnchor,
        // Control flow
        condition, elseChild, cases, fallback, items,
        // Gesture / focus
        behavior, onClick, onPointerDown, onPointerMove, onPointerUp, cursor,
        onEnter, onExit, onKeyDown, onKeyUp, onFocus, onBlur,
        // Virtual app view
        background, errorView,
        // Reactive subscribe
        readables,
        // Misc
        queryKey,
    }

    // Collision renames — the JS method name differs from the prop key it
    // writes (`build` would collide with the terminal `.build()`).
    #[allow(non_snake_case)]
    pub fn itemBuilder(
        this: &JsValue,
        args: &[JsValue],
        context: &mut Context,
    ) -> JsResult<JsValue> {
        builder_set_prop(this, args, context, "build")
    }

    #[allow(non_snake_case)]
    pub fn rowBuilder(
        this: &JsValue,
        args: &[JsValue],
        context: &mut Context,
    ) -> JsResult<JsValue> {
        builder_set_prop(this, args, context, "build")
    }

    #[allow(non_snake_case)]
    pub fn headerBuilder(
        this: &JsValue,
        args: &[JsValue],
        context: &mut Context,
    ) -> JsResult<JsValue> {
        builder_set_prop(this, args, context, "buildHeader")
    }

    // `$` is not a valid Rust identifier character — these methods get
    // Rust-side names with the key written explicitly.
    #[allow(non_snake_case)]
    pub fn app_handle(
        this: &JsValue,
        args: &[JsValue],
        context: &mut Context,
    ) -> JsResult<JsValue> {
        builder_set_prop(this, args, context, "app$")
    }

    #[allow(non_snake_case)]
    pub fn onUpdateMutation(
        this: &JsValue,
        args: &[JsValue],
        context: &mut Context,
    ) -> JsResult<JsValue> {
        builder_set_prop(this, args, context, "onUpdate$")
    }
}
