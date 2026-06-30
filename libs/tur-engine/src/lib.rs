pub mod core;
pub mod elements;
pub mod handlers;
pub mod renderer;

pub mod error;

use std::rc::Rc;
use std::time::Duration;

use boa_engine::context::time::FixedClock;
use boa_engine::Context;
use boa_engine::Source;
use std::path::Path;
use error::TurError;

use core::app::TurAppInternal;
use core::bridge::init_bridge;
use core::bridge::TurJobExecutor;
use core::element::{ElementNodeId, NodeId};
use core::elements::AnyElement;
use core::host_api::HostApi;
#[cfg(feature = "trace")]
use core::elements::NodeTreeData;
use elements::editable_text::EditableTextElement;

pub struct TurApp {
    boa_context: Context,
    internal: TurAppInternal,
    executor: Rc<TurJobExecutor>,
}

impl TurApp {
    pub fn new(
        renderer: Box<dyn core::render::Renderer>,
        font_loader: Box<dyn core::fonts::FontLoader>,
        host_api: Box<dyn HostApi>,
    ) -> Result<Self, TurError> {
        let clock = Rc::new(FixedClock::from_millis(0));
        let executor = Rc::new(TurJobExecutor::new());
        let mut boa_context = Context::builder()
            .clock(clock.clone())
            .job_executor(executor.clone())
            .build()
            .expect("failed to build boa context");

        let BridgeResult { internal, executor } =
            init_bridge(&mut boa_context, renderer, font_loader, clock, host_api, executor);

        tracing::info!("TurApp initialized");

        Ok(TurApp {
            boa_context,
            internal,
            executor,
        })
    }

    pub fn load_js(&mut self, source: &str) -> Result<(), TurError> {
        tracing::info!("load_js: evaluating bundle ({} bytes)", source.len());
        self.boa_context
            .eval(Source::from_bytes(source).with_path(Path::new("bundle.js")))
            .map_err(|e| {
                tracing::error!("JS eval error: {e}");
                TurError::JsEval(e)
            })?;
        if let Err(e) = self.executor.drain(&mut self.boa_context) {
            tracing::error!("load_js drain error: {e}");
        }
        tracing::info!("load_js: bundle evaluated successfully");
        Ok(())
    }

    pub fn eval_js(&mut self, source: &str) -> Result<String, TurError> {
        let result = self
            .boa_context
            .eval(Source::from_bytes(source))
            .map_err(TurError::JsEval)?;
        let s = result
            .as_string()
            .map(|s| s.to_std_string_escaped())
            .unwrap_or_else(|| result.display().to_string());
        Ok(s)
    }

    /// Register a host function on `globalThis.__turHost.<name>`.
    ///
    /// Generic, compiler-agnostic extension point: lets an embedder (e.g.
    /// `tur-wasm`) expose native services to JS without polluting the core
    /// `__tur` view namespace. The `__turHost` object is created on first
    /// call. The embedder owns any heavy dependencies (e.g. swc) — tur-engine
    /// itself only provides this hook.
    pub fn register_host_fn(
        &mut self,
        name: &str,
        length: usize,
        f: boa_engine::native_function::NativeFunction,
    ) -> Result<(), boa_engine::JsError> {
        use boa_engine::js_string;
        use boa_engine::object::FunctionObjectBuilder;
        use boa_engine::property::PropertyDescriptor;

        let host_key = js_string!("__turHost");

        // Get-or-create globalThis.__turHost.
        let host_obj = {
            let global = self.boa_context.global_object();
            let existing = global.get(host_key.clone(), &mut self.boa_context)?;
            if let Some(obj) = existing.as_object() {
                obj.clone()
            } else {
                let proto = self
                    .boa_context
                    .intrinsics()
                    .constructors()
                    .object()
                    .prototype();
                let obj = boa_engine::object::JsObject::from_proto_and_data(proto, ());
                let desc = PropertyDescriptor::builder()
                    .value(obj.clone())
                    .writable(true)
                    .enumerable(false)
                    .configurable(true)
                    .build();
                global.insert_property(host_key.clone(), desc);
                obj
            }
        };

        // Build the function and attach it to __turHost.<name>.
        let fn_obj = FunctionObjectBuilder::new(self.boa_context.realm(), f)
            .name(js_string!(name))
            .length(length)
            .build();
        let desc = PropertyDescriptor::builder()
            .value(fn_obj)
            .writable(true)
            .enumerable(false)
            .configurable(true)
            .build();
        host_obj.insert_property(js_string!(name), desc);

        tracing::info!("registered host function __turHost.{name}");
        Ok(())
    }

    /// Register a function on `globalThis.__tur.<name>` — the core view
    /// namespace, set up by `init_bridge`. Lets an embedder (e.g. tur-wasm)
    /// add capability functions that JS calls as `__tur.<name>(...)`, alongside
    /// the built-in view factories. The embedder owns any heavy dependencies
    /// (e.g. reqwest_wasm); tur-engine only provides this hook.
    pub fn register_tur_fn(
        &mut self,
        name: &str,
        length: usize,
        f: boa_engine::native_function::NativeFunction,
    ) -> Result<(), boa_engine::JsError> {
        use boa_engine::js_string;
        use boa_engine::object::FunctionObjectBuilder;
        use boa_engine::property::PropertyDescriptor;

        let tur_key = js_string!("__tur");

        let global = self.boa_context.global_object();
        let tur_obj = match global.get(tur_key.clone(), &mut self.boa_context) {
            Ok(v) if v.as_object().is_some() => v.as_object().unwrap().clone(),
            _ => {
                // `__tur` should always exist after init_bridge; create a
                // bare fallback so a misordered call doesn't panic.
                let proto = self
                    .boa_context
                    .intrinsics()
                    .constructors()
                    .object()
                    .prototype();
                let obj = boa_engine::object::JsObject::from_proto_and_data(proto, ());
                let desc = PropertyDescriptor::builder()
                    .value(obj.clone())
                    .writable(true)
                    .enumerable(false)
                    .configurable(true)
                    .build();
                global.insert_property(tur_key.clone(), desc);
                obj
            }
        };

        let fn_obj = FunctionObjectBuilder::new(self.boa_context.realm(), f)
            .name(js_string!(name))
            .length(length)
            .build();
        let desc = PropertyDescriptor::builder()
            .value(fn_obj)
            .writable(true)
            .enumerable(false)
            .configurable(true)
            .build();
        tur_obj.insert_property(js_string!(name), desc);

        tracing::info!("registered tur function __tur.{name}");
        Ok(())
    }

    pub fn spawn_loop_once(&mut self, advanced_time: Duration) -> Result<(), TurError> {
        self.internal
            .app_context
            .borrow()
            .shell
            .forward(advanced_time.as_millis() as u64);
        self.internal.flush(&mut self.boa_context)?;
        Ok(())
    }

    /// Briefly expose the boa context so embedder-side callbacks (e.g. the
    /// resolved `clipboardReadText` callbacks) can be invoked. Used by
    /// tur-wasm's frame loop after each `spawn_loop_once`.
    pub fn with_boa_context<R>(&mut self, f: impl FnOnce(&mut Context) -> R) -> R {
        f(&mut self.boa_context)
    }

    pub fn push_event(&self, event: core::event::AppEvent) {
        self.internal
            .app_context
            .borrow_mut()
            .event_queue
            .push(event);
    }

    /// Structured dev-tool snapshot of the root node. Returns `None` if no
    /// tree is mounted. Children are returned as bare ids; iterate with
    /// `dev_tool_get_element`.
    pub fn dev_tool_element_tree(&self) -> Option<core::elements::DevNodeData> {
        let tree = self.internal.js_context.element_tree.borrow();
        let root_id = tree.root_element_id()?;
        tree.dev_tool_node(root_id.into())
    }

    /// Structured dev-tool snapshot of an arbitrary node by id.
    pub fn dev_tool_get_element(
        &self,
        id: core::element::NodeId,
    ) -> Option<core::elements::DevNodeData> {
        self.internal.js_context.element_tree.borrow().dev_tool_node(id)
    }

    /// Returns any text written to the clipboard via `AppEvent::ClipboardWrite`
    /// since the last call. Embedders (tur-wasm) drain this each frame and
    /// forward the text to the real system clipboard (e.g.
    /// `navigator.clipboard.writeText`). `None` means no clipboard write is
    /// pending.
    pub fn take_clipboard_write(&self) -> Option<String> {
        self.internal
            .app_context
            .borrow()
            .pending_clipboard_write
            .borrow_mut()
            .take()
    }

    pub fn query_element(&self, key: &[&str]) -> Option<NodeId> {
        self.internal
            .js_context
            .element_tree
            .borrow()
            .query_element(key)
    }

    pub fn focused_element(&self) -> Option<ElementNodeId> {
        self.internal.js_context.focus_manager.borrow().focused()
    }

    pub fn with_element<R>(
        &self,
        id: ElementNodeId,
        cb: impl FnOnce(&AnyElement) -> R,
    ) -> Option<R> {
        let tree = self.internal.js_context.element_tree.borrow();
        let node = tree.get_element(id)?;
        let element = node.element.as_ref()?;
        Some(cb(element))
    }

    pub fn focused_cursor_rect(&self) -> Option<(f64, f64, f64, f64)> {
        let focused_id = self.focused_element()?;
        let tree = self.internal.js_context.element_tree.borrow();

        let mut abs_x = 0.0f64;
        let mut abs_y = 0.0f64;
        let mut current: Option<NodeId> = Some(focused_id.into());
        while let Some(id) = current {
            let node = tree.get_element(ElementNodeId::new(id.as_u64()))?;
            abs_x += node.computed_layout.offset.x;
            abs_y += node.computed_layout.offset.y;
            current = node.parent;
        }

        let node = tree.get_element(focused_id)?;
        let element = node.element.as_ref()?;
        let editable_el = element.cast::<EditableTextElement>()?;
        let layout_data = editable_el.cached_layout.as_ref()?;

        let cursor_byte = editable_el.cursor_position();

        let (cursor_x, _) = layout_data.cursor_xy_at(cursor_byte);
        let line_idx = layout_data.line_index_for_byte(cursor_byte);
        let line_info = &layout_data.line_infos[line_idx];

        Some((
            abs_x + cursor_x as f64,
            abs_y + line_info.top as f64,
            2.0,
            line_info.height as f64,
        ))
    }

    pub fn focused_is_editable(&self) -> bool {
        use core::focus::helper;
        let tree = self.internal.js_context.element_tree.borrow();
        let focus = self.internal.js_context.focus_manager.borrow();
        helper::focused_is_editable(&tree, &focus)
    }

    #[cfg(feature = "trace")]
    pub fn element_tree(&self) -> std::cell::Ref<'_, NodeTreeData> {
        self.internal.js_context.element_tree.borrow()
    }

    pub fn render_to_pixels(&mut self) -> Option<Vec<u8>> {
        self.internal.app_context.borrow_mut().render_to_pixels()
    }
}

use core::bridge::BridgeResult;
