//! `builtin:tur/std` clipboard bridge: `clipboardReadText`/`clipboardWriteText`
//! as Promise-returning fns.
//!
//! The bridge fns are closures (not ctx-bound fn pointers) because they
//! capture a `Rc<dyn Clipboard>` (provided by the plugin builder) and a
//! `Rc<AsyncExecutor>` (provided by the engine via `PluginContext`). The
//! pattern:
//!
//! 1. Bridge fn creates a pending `JsPromise` synchronously and returns it.
//! 2. Spawns a future via the executor that calls `clipboard.read_text()`
//!    (or `write_text`) — this is the only async part.
//! 3. On completion, the future pushes a `Completion` closure into the
//!    executor that runs under `&mut Context` on the next `flush` and
//!    resolves the promise.
//!
//! Promise settlement enqueues a PromiseJob; boa's `executor.drain` (called
//! right after `tick`/`drain_completions` in `flush`) runs the `.then`
//! callbacks, which can `set()` reactive atoms that drive re-layout.

use std::rc::Rc;

use boa_engine::native_function::NativeFunction;
use boa_engine::object::builtins::JsPromise;
use boa_engine::{js_string, JsArgs, JsValue};

use tur_engine::core::async_::AsyncExecutor;

use crate::platform::Clipboard;

/// Build the clipboard bridge closures for `builtin:tur/std`.
///
/// Returns `(name, length, NativeFunction)` entries ready for
/// [`tur_engine::core::plugin::PluginContext::register_module`]'s `closures`
/// parameter. Each closure captures clones of `clipboard` and `executor`.
pub fn closures(
    clipboard: Rc<dyn Clipboard>,
    executor: Rc<AsyncExecutor>,
) -> Vec<(&'static str, usize, NativeFunction)> {
    let read = build_clipboard_read(clipboard.clone(), executor.clone());
    let write = build_clipboard_write(clipboard, executor);
    vec![("clipboardReadText", 0, read), ("clipboardWriteText", 1, write)]
}

fn build_clipboard_read(
    clipboard: Rc<dyn Clipboard>,
    executor: Rc<AsyncExecutor>,
) -> NativeFunction {
    // SAFETY: captures are pure Rust state (`Rc<dyn Clipboard>`,
    // `Rc<AsyncExecutor>`) — no boa GC-traceable types. Sound to use
    // `from_closure` (the unsafe invariant is "no traceable captures").
    unsafe {
        NativeFunction::from_closure(move |_this, _args, ctx| {
            let (promise, resolvers) = JsPromise::new_pending(ctx);
            let clipboard = clipboard.clone();
            let executor = executor.clone();
            let executor_for_complete = executor.clone();
            executor.spawn_detached(async move {
                let text = clipboard.read_text().await;
                // Push a completion closure that resolves the promise under
                // `&mut Context`. Runs on the next `flush`'s `drain_completions`
                // pass, which is followed by boa's `executor.drain` — so the
                // promise's `.then` callbacks fire in the same iteration.
                executor_for_complete.complete(Box::new(move |ctx| {
                    let v = JsValue::from(js_string!(text.as_str()));
                    resolvers.resolve.call(&JsValue::undefined(), &[v], ctx)?;
                    Ok(())
                }));
            });
            Ok(promise.into())
        })
    }
}

fn build_clipboard_write(
    clipboard: Rc<dyn Clipboard>,
    executor: Rc<AsyncExecutor>,
) -> NativeFunction {
    // SAFETY: see `build_clipboard_read`.
    unsafe {
        NativeFunction::from_closure(move |_this, args, ctx| {
            let (promise, resolvers) = JsPromise::new_pending(ctx);
            let text = args
                .get_or_undefined(0)
                .as_string()
                .map(|s| s.to_std_string_escaped())
                .unwrap_or_default();
            let clipboard = clipboard.clone();
            let executor = executor.clone();
            let executor_for_complete = executor.clone();
            executor.spawn_detached(async move {
                clipboard.write_text(text).await;
                executor_for_complete.complete(Box::new(move |ctx| {
                    resolvers
                        .resolve
                        .call(&JsValue::undefined(), &[JsValue::undefined()], ctx)?;
                    Ok(())
                }));
            });
            Ok(promise.into())
        })
    }
}
