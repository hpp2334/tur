//! `tur:clipboard` bridge: a `clipboard` object with `readText` /
//! `writeText` methods, each returning a `Promise`.
//!
//! The bridge fns are ctx-bound `Ptr`s that look up the
//! [`Clipboard`](super::capability::Clipboard) capability from `TurJsContext`'s
//! capability registry (populated by the embedder via
//! `TurRuntimeBuilder::capability(Clipboard::new(...))`).
//!
//! Promise settlement flow:
//!
//! 1. Bridge fn creates a pending `JsPromise` synchronously and returns it.
//! 2. Spawns a future via [`WorkerScheduler::spawn_local`] that calls
//!    `clipboard.read_text()` (or `write_text`) — this is the only async
//!    part.
//! 3. On completion, the future pushes a `Completion` closure into the
//!    [`CompletionHandle`] that runs under `&mut Context` on the next
//!    `flush` and resolves the promise.
//!
//! Promise settlement enqueues a PromiseJob; boa's `executor.drain` (called
//! right after `drain_completions` in `flush`) runs the `.then`
//! callbacks, which can `set()` reactive atoms that drive re-layout.

use boa_engine::js_string;
use boa_engine::object::JsObject;
use boa_engine::object::builtins::JsPromise;
use boa_engine::{Context, JsArgs, JsError, JsNativeError, JsResult, JsValue};

use crate::core::js_runtime::helpers::{Ptr, extract_js_ctx};
use crate::core::js_runtime::module_loader::bound_native;

use super::capability::Clipboard;

/// Bridge function tables entries for `tur:clipboard`.
///
/// Returns an empty list because the module's only export is a `clipboard`
/// *object* (built by [`build_clipboard_object`]) registered as a const.
/// Listed here for symmetry with other bridge files; not currently consumed.
pub(in crate::builtin_plugins) fn fns() -> Vec<crate::core::js_runtime::helpers::FnEntry> {
    Vec::new()
}

/// Build the `clipboard` JS object: `{ readText(): Promise<string>,
/// writeText(text: string): Promise<void> }`. Each method is a ctx-bound
/// native fn (via [`bound_native`]) so it can `extract_js_ctx(args)` and look
/// up its capability slot.
pub(in crate::builtin_plugins) fn build_clipboard_object(
    context: &mut Context,
    ctx_value: JsValue,
) -> JsValue {
    let read = bound_native(
        context,
        ctx_value.clone(),
        tur_clipboard_read_text as Ptr,
        0,
        "readText",
    );
    let write = bound_native(
        context,
        ctx_value,
        tur_clipboard_write_text as Ptr,
        1,
        "writeText",
    );
    let obj = JsObject::with_object_proto(context.intrinsics());
    let _ = obj.create_data_property(js_string!("readText"), JsValue::from(read), context);
    let _ = obj.create_data_property(js_string!("writeText"), JsValue::from(write), context);
    obj.into()
}

/// `clipboard.readText(): Promise<string>` — reads text from the platform
/// clipboard. Resolves with the text (empty string if denied/unavailable).
fn tur_clipboard_read_text(
    _this: &JsValue,
    args: &[JsValue],
    ctx: &mut Context,
) -> JsResult<JsValue> {
    let js_ctx = extract_js_ctx(args)?;
    let clipboard = js_ctx
        .capability()
        .of::<Clipboard>()
        .ok_or_else(|| JsError::from(JsNativeError::typ().with_message("no clipboard capability")))?
        .backend()
        .clone();
    let completion_handle = js_ctx.completion_handle();

    let (promise, resolvers) = JsPromise::new_pending(ctx);
    let _ = js_ctx.spawn_local(|_aw| async move {
        let text = clipboard.read_text().await;
        // Push a completion closure that resolves the promise under
        // `&mut Context`. Runs on the next `flush`'s `drain_completions`
        // pass, which is followed by boa's `executor.drain` — so the
        // promise's `.then` callbacks fire in the same iteration.
        completion_handle.push(Box::new(move |ctx| {
            let v = JsValue::from(js_string!(text.as_str()));
            resolvers.resolve.call(&JsValue::undefined(), &[v], ctx)?;
            Ok(())
        }));
    });
    Ok(promise.into())
}

/// `clipboard.writeText(text: string): Promise<void>` — writes text to the
/// platform clipboard. Resolves when the write has been acknowledged.
fn tur_clipboard_write_text(
    _this: &JsValue,
    args: &[JsValue],
    ctx: &mut Context,
) -> JsResult<JsValue> {
    let js_ctx = extract_js_ctx(args)?;
    let clipboard = js_ctx
        .capability()
        .of::<Clipboard>()
        .ok_or_else(|| JsError::from(JsNativeError::typ().with_message("no clipboard capability")))?
        .backend()
        .clone();
    let completion_handle = js_ctx.completion_handle();

    let (promise, resolvers) = JsPromise::new_pending(ctx);
    let text = args
        .get_or_undefined(1)
        .as_string()
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();
    let _ = js_ctx.spawn_local(|_aw| async move {
        clipboard.write_text(text).await;
        completion_handle.push(Box::new(move |ctx| {
            resolvers
                .resolve
                .call(&JsValue::undefined(), &[JsValue::undefined()], ctx)?;
            Ok(())
        }));
    });
    Ok(promise.into())
}
