//! `tur:filepicker` bridge: a `filePicker` object with `pick` /
//! `saveFile` methods, each returning a `Promise`.
//!
//! The bridge fns are ctx-bound `Ptr`s that look up the
//! [`FilePicker`](crate::FilePicker) capability from `TurJsContext`'s
//! capability registry (populated by the embedder via
//! `TurRuntimeBuilder::capability(FilePicker::new(...))`).
//!
//! Promise settlement flow:
//!
//! 1. Bridge fn creates a pending `JsPromise` synchronously and returns it.
//! 2. Spawns a future via the executor that calls `FilePicker::pick(opts)`
//!    (or `save`) — this is the only async part.
//! 3. On completion, the future pushes a `Completion` closure into the
//!    executor that runs under `&mut Context` on the next `flush` and
//!    resolves the promise (building the JS `{ name, bytes, type, size }`
//!    objects + `ArrayBuffer`s there, where the boa `Context` is available).

use boa_engine::js_string;
use boa_engine::object::JsObject;
use boa_engine::object::builtins::{JsArray, JsArrayBuffer, JsPromise};
use boa_engine::{Context, JsArgs, JsError, JsNativeError, JsResult, JsValue};

use tur_engine::core::js_runtime::helpers::{FnEntry, Ptr, extract_js_ctx};
use tur_engine::core::js_runtime::module_loader::bound_native;

use crate::{FilePicker, PickOptions, PickedFile, SaveOptions};

/// Bridge function table entries for `tur:filepicker`.
///
/// Returns an empty list because the module's only export is a `filePicker`
/// *object* (built by [`build_filepicker_object`]) registered as a const.
pub fn fns() -> Vec<FnEntry> {
    Vec::new()
}

/// Build the `filePicker` JS object:
/// `{ pick(opts?): Promise<PickedFile[]>, saveFile(name, bytes, opts?): Promise<void> }`.
/// Each method is a ctx-bound native fn (via [`bound_native`]) so it can
/// `extract_js_ctx(args)` and look up its capability slot.
pub fn build_filepicker_object(context: &mut Context, ctx_value: JsValue) -> JsValue {
    let pick = bound_native(
        context,
        ctx_value.clone(),
        tur_filepicker_pick as Ptr,
        1,
        "pick",
    );
    let save = bound_native(
        context,
        ctx_value,
        tur_filepicker_save as Ptr,
        2,
        "saveFile",
    );
    let obj = JsObject::with_object_proto(context.intrinsics());
    let _ = obj.create_data_property(js_string!("pick"), JsValue::from(pick), context);
    let _ = obj.create_data_property(js_string!("saveFile"), JsValue::from(save), context);
    obj.into()
}

/// `filePicker.pick(opts?): Promise<PickedFile[]>` — opens the platform file
/// picker. Resolves with the picked files (empty array if cancelled/denied).
fn tur_filepicker_pick(_this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let js_ctx = extract_js_ctx(args)?;
    let picker = js_ctx
        .capability()
        .of::<FilePicker>()
        .ok_or_else(|| {
            JsError::from(JsNativeError::typ().with_message("no filepicker capability"))
        })?
        .backend()
        .clone();
    let completion_handle = js_ctx.completion_handle();

    let (promise, resolvers) = JsPromise::new_pending(ctx);
    let opts = parse_pick_opts(args, ctx);
    let _ = js_ctx.spawn_local(|_aw| async move {
        let files = picker.pick(opts).await;
        completion_handle.push(Box::new(move |ctx| {
            let arr = JsArray::new(ctx)?;
            for f in files {
                let o = build_picked_file_object(&f, ctx)?;
                arr.push(o, ctx)?;
            }
            resolvers
                .resolve
                .call(&JsValue::undefined(), &[arr.into()], ctx)?;
            Ok(())
        }));
    });
    Ok(promise.into())
}

/// `filePicker.saveFile(name, bytes, opts?): Promise<void>` — persists
/// `bytes` under file name `name` via the platform save dialog / download.
fn tur_filepicker_save(_this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let js_ctx = extract_js_ctx(args)?;
    let picker = js_ctx
        .capability()
        .of::<FilePicker>()
        .ok_or_else(|| {
            JsError::from(JsNativeError::typ().with_message("no filepicker capability"))
        })?
        .backend()
        .clone();
    let completion_handle = js_ctx.completion_handle();

    let (promise, resolvers) = JsPromise::new_pending(ctx);
    let name = args
        .get_or_undefined(1)
        .as_string()
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_else(|| "download".to_string());
    let bytes = args
        .get_or_undefined(2)
        .as_object()
        .and_then(|o| JsArrayBuffer::from_object(o.clone()).ok())
        .and_then(|ab| ab.to_vec())
        .unwrap_or_default();
    let opts = parse_save_opts(args, ctx);
    let _ = js_ctx.spawn_local(|_aw| async move {
        picker.save(name, bytes, opts).await;
        completion_handle.push(Box::new(move |ctx| {
            resolvers
                .resolve
                .call(&JsValue::undefined(), &[JsValue::undefined()], ctx)?;
            Ok(())
        }));
    });
    Ok(promise.into())
}

/// Parse the JS `{ accept?, multiple? }` opts object from `args[1]` (the
/// user's opts arg; `args[0]` is the bound ctx).
fn parse_pick_opts(args: &[JsValue], ctx: &mut Context) -> PickOptions {
    let mut opts = PickOptions::default();
    let Some(obj) = args.get_or_undefined(1).as_object() else {
        return opts;
    };
    if let Ok(v) = obj.get(js_string!("multiple"), ctx) {
        opts.multiple = v.as_boolean().unwrap_or(false);
    }
    if let Ok(arr_v) = obj.get(js_string!("accept"), ctx)
        && let Some(arr_obj) = arr_v.as_object()
        && let Ok(arr) = JsArray::from_object(arr_obj.clone())
        && let Ok(len) = arr.length(ctx)
    {
        for i in 0..len as i64 {
            if let Ok(v) = arr.at(i, ctx)
                && let Some(s) = v.as_string()
            {
                opts.accept.push(s.to_std_string_escaped());
            }
        }
    }
    opts
}

/// Parse the JS `{ accept? }` opts object from `args[3]` (the user's opts
/// arg for `saveFile`; `args[0]` is the bound ctx, `args[1]` = name,
/// `args[2]` = bytes).
fn parse_save_opts(args: &[JsValue], ctx: &mut Context) -> SaveOptions {
    let mut opts = SaveOptions::default();
    let Some(obj) = args.get_or_undefined(3).as_object() else {
        return opts;
    };
    if let Ok(arr_v) = obj.get(js_string!("accept"), ctx)
        && let Some(arr_obj) = arr_v.as_object()
        && let Ok(arr) = JsArray::from_object(arr_obj.clone())
        && let Ok(len) = arr.length(ctx)
    {
        for i in 0..len as i64 {
            if let Ok(v) = arr.at(i, ctx)
                && let Some(s) = v.as_string()
            {
                opts.accept.push(s.to_std_string_escaped());
            }
        }
    }
    opts
}

/// Build the JS `{ name, bytes: ArrayBuffer, type, size }` object for one
/// [`PickedFile`]. The `ArrayBuffer` is constructed here (under `&mut
/// Context`) from the copied bytes.
fn build_picked_file_object(f: &PickedFile, ctx: &mut Context) -> JsResult<JsValue> {
    use boa_engine::object::builtins::AlignedVec;

    let size = f.bytes.len();
    let mime = f.mime_type.clone().unwrap_or_default();
    let o = JsObject::with_object_proto(ctx.intrinsics());
    let _ = o.create_data_property(
        js_string!("name"),
        JsValue::from(js_string!(f.name.as_str())),
        ctx,
    );
    if let Ok(ab) = JsArrayBuffer::from_byte_block(AlignedVec::from_iter(0, f.bytes.clone()), ctx) {
        let _ = o.create_data_property(js_string!("bytes"), JsValue::from(ab), ctx);
    }
    let _ = o.create_data_property(
        js_string!("type"),
        JsValue::from(js_string!(mime.as_str())),
        ctx,
    );
    let _ = o.create_data_property(js_string!("size"), JsValue::from(size as f64), ctx);
    Ok(o.into())
}
