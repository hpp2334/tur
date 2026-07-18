use std::cell::{Cell, RefCell};
use std::rc::Rc;

use boa_engine::class::{Class, ClassBuilder};
use boa_engine::js_string;
use boa_engine::native_function::NativeFunction;
use boa_engine::object::JsObject;
use boa_engine::property::Attribute;
use boa_engine::{Context, JsArgs, JsNativeError, JsResult, JsValue};
use boa_gc::{Finalize, Trace};

use crate::core::bridge::BoaOpaque;
use crate::core::bridge::{TurJsContext, TurNodeHandle};
use crate::core::mutation::{extract_mutation_from_opts, MutationHandle, PendingMutationInvocationQueue};
use crate::core::element::ElementNodeId;
use crate::core::scroll::ScrollEvent;
use crate::elements::scroll_view::ScrollViewElement;

#[derive(Trace, Finalize, boa_engine::JsData)]
#[boa_gc(unsafe_empty_trace)]
pub struct ScrollController {
    pub(crate) offset: f64,
    pub(crate) max_scroll_extent: f64,
    pub(crate) viewport_dimension: f64,
    pub(crate) on_scroll: Option<MutationHandle<ScrollEvent>>,
    pub(crate) handle: Option<JsObject>,
    /// The scroll-view node this controller is bound to. Set at build time by
    /// `ScrollViewView::build` (the `_attach` JS path is the legacy
    /// fallback). `jumpTo`/drag use this to locate the scroll element.
    pub(crate) bound_node: Option<ElementNodeId>,
    pub(crate) element_tree:
        Option<crate::core::elements::NodeTree>,
    pub(crate) mutation_queue: Option<Rc<RefCell<PendingMutationInvocationQueue>>>,
    pub(crate) dirty_flag: Option<Rc<Cell<bool>>>,
    pub(crate) pending_initial_offset: Option<f64>,
}

impl ScrollController {
    pub fn new() -> Self {
        Self {
            offset: 0.0,
            max_scroll_extent: 0.0,
            viewport_dimension: 0.0,
            on_scroll: None,
            handle: None,
            bound_node: None,
            element_tree: None,
            mutation_queue: None,
            dirty_flag: None,
            pending_initial_offset: None,
        }
    }

    fn node_id(&self) -> Option<ElementNodeId> {
        if let Some(n) = self.bound_node {
            return Some(n);
        }
        let handle_obj = self.handle.as_ref()?;
        let handle = BoaOpaque::<TurNodeHandle>::wrap(handle_obj)?;
        Some(handle.id)
    }
}

impl Default for ScrollController {
    fn default() -> Self {
        Self::new()
    }
}

macro_rules! controller_getter {
    ($class:expr, $name:expr, $body:expr) => {
        let getter = NativeFunction::from_fn_ptr($body)
            .to_js_function($class.context().realm());
        $class.accessor(
            js_string!($name),
            Some(getter),
            None,
            Attribute::default(),
        );
    };
}

impl Class for ScrollController {
    const NAME: &'static str = "ScrollController";
    const LENGTH: usize = 1;

    fn data_constructor(
        _new_target: &JsValue,
        args: &[JsValue],
        ctx: &mut Context,
    ) -> JsResult<Self> {
        let mut ctrl = Self::new();
        if let Some(opts) = args.get_or_undefined(0).as_object() {
            ctrl.on_scroll = extract_mutation_from_opts(&opts, "onScroll", ctx);
            if let Ok(val) = opts.get(js_string!("initialOffset"), ctx)
                && let Some(n) = val.as_number() {
                    ctrl.pending_initial_offset = Some(n);
                }
        }
        Ok(ctrl)
    }

    fn init(class: &mut ClassBuilder<'_>) -> JsResult<()> {
        controller_getter!(class, "offset", |this, _, _| {
            let obj = this.as_object().ok_or_else(|| {
                JsNativeError::typ().with_message("invalid this")
            })?;
            let ctrl = obj
                .downcast_ref::<ScrollController>()
                .ok_or_else(|| JsNativeError::typ().with_message("invalid this"))?;
            Ok(JsValue::from(ctrl.offset))
        });

        controller_getter!(class, "maxScrollExtent", |this, _, _| {
            let obj = this.as_object().ok_or_else(|| {
                JsNativeError::typ().with_message("invalid this")
            })?;
            let ctrl = obj
                .downcast_ref::<ScrollController>()
                .ok_or_else(|| JsNativeError::typ().with_message("invalid this"))?;
            Ok(JsValue::from(ctrl.max_scroll_extent))
        });

        controller_getter!(class, "viewportDimension", |this, _, _| {
            let obj = this.as_object().ok_or_else(|| {
                JsNativeError::typ().with_message("invalid this")
            })?;
            let ctrl = obj
                .downcast_ref::<ScrollController>()
                .ok_or_else(|| JsNativeError::typ().with_message("invalid this"))?;
            Ok(JsValue::from(ctrl.viewport_dimension))
        });

        class.method(
            js_string!("jumpTo"),
            1,
            NativeFunction::from_fn_ptr(|this, args, _| {
                let obj = this.as_object().ok_or_else(|| {
                    JsNativeError::typ().with_message("invalid this")
                })?;
                let mut ctrl = obj
                    .downcast_mut::<ScrollController>()
                    .ok_or_else(|| JsNativeError::typ().with_message("invalid this"))?;

                let target_offset = args.get_or_undefined(0).as_number().unwrap_or(0.0);

                let element_tree_rc = ctrl.element_tree.clone();
                let dirty_flag = ctrl.dirty_flag.clone();
                let mutation_queue = ctrl.mutation_queue.clone();
                let node_id = ctrl.node_id();
                let on_scroll = ctrl.on_scroll;

                let Some(element_tree_rc) = element_tree_rc else {
                    return Ok(JsValue::undefined());
                };
                let Some(dirty_flag) = dirty_flag else {
                    return Ok(JsValue::undefined());
                };
                let Some(node_id) = node_id else {
                    return Ok(JsValue::undefined());
                };

                let mut tree = element_tree_rc.borrow_mut();
                let Some(node) = tree.get_element_mut(node_id) else {
                    return Ok(JsValue::undefined());
                };
                let Some(ref mut element) = node.element else {
                    return Ok(JsValue::undefined());
                };
                let Some(sv) = element.cast_mut::<ScrollViewElement>() else {
                    return Ok(JsValue::undefined());
                };

                let max = sv.position.max_scroll_extent();
                let clamped = target_offset.clamp(0.0, max);
                sv.position.correct_pixels(clamped);

                let vp = sv.viewport_size();
                let dim = match sv.axis() {
                    crate::core::layout::Axis::Vertical => vp.height,
                    crate::core::layout::Axis::Horizontal => vp.width,
                };
                let new_offset = sv.position.pixels();
                tree.mark_dirty(node_id.into());
                drop(tree);

                ctrl.offset = new_offset;
                ctrl.max_scroll_extent = max;
                ctrl.viewport_dimension = dim;
                dirty_flag.set(true);

                if let Some(queue_rc) = mutation_queue
                    && let Some(m) = on_scroll {
                        queue_rc.borrow_mut().push(
                            m,
                            ScrollEvent {
                                offset: ctrl.offset,
                                max_extent: ctrl.max_scroll_extent,
                                viewport_dimension: ctrl.viewport_dimension,
                            },
                        );
                    }

                Ok(JsValue::undefined())
            }),
        );

        class.method(
            js_string!("_attach"),
            2,
            NativeFunction::from_fn_ptr(|this, args, _| {
                let obj = this.as_object().ok_or_else(|| {
                    JsNativeError::typ().with_message("invalid this")
                })?;
                let mut ctrl = obj
                    .downcast_mut::<ScrollController>()
                    .ok_or_else(|| JsNativeError::typ().with_message("invalid this"))?;

                if let Some(handle_obj) = args.get_or_undefined(0).as_object()
                    && BoaOpaque::<TurNodeHandle>::wrap(&handle_obj).is_some() {
                        ctrl.handle = Some(handle_obj.clone());
                    }

                if let Some(ctx_obj) = args.get_or_undefined(1).as_object()
                    && let Some(js_ctx) =
                        BoaOpaque::<TurJsContext>::wrap(&ctx_obj)
                    {
                        ctrl.element_tree = Some(js_ctx.element_tree.clone());
                        ctrl.mutation_queue = Some(js_ctx.mutation_queue.clone());
                        ctrl.dirty_flag = Some(js_ctx.dirty.clone());
                    }

                Ok(JsValue::undefined())
            }),
        );

        Ok(())
    }
}
