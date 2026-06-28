use std::cell::{Cell, RefCell};
use std::rc::Rc;

use boa_engine::class::{Class, ClassBuilder};
use boa_engine::js_string;
use boa_engine::native_function::NativeFunction;
use boa_engine::object::JsObject;
use boa_engine::property::Attribute;
use boa_engine::{Context, JsArgs, JsNativeError, JsResult, JsValue};
use boa_gc::{Finalize, Trace};

use crate::core::bridge::{BoaOpaque, TurJsContext, TurNodeHandle};
use crate::core::edgy_event::{extract_mutation_from_opts, EdgyMutation, PendingMutationInvocationQueue};
use crate::core::element::{ElementNodeId, NodeId};
use crate::core::scroll::ScrollEvent;
use crate::elements::lazy_list::VisibleRangeChangeEvent;
use crate::elements::LazyListElement;

#[derive(Trace, Finalize, boa_engine::JsData)]
#[boa_gc(unsafe_empty_trace)]
pub struct LazyListController {
    pub(crate) offset: f64,
    pub(crate) max_scroll_extent: f64,
    pub(crate) viewport_dimension: f64,
    pub(crate) on_scroll: Option<EdgyMutation<ScrollEvent>>,
    pub(crate) on_visible_range_change: Option<EdgyMutation<VisibleRangeChangeEvent>>,
    pub(crate) handle: Option<JsObject>,
    pub(crate) element_tree:
        Option<Rc<RefCell<crate::core::elements::ElementTree>>>,
    pub(crate) mutation_queue: Option<Rc<RefCell<PendingMutationInvocationQueue>>>,
    pub(crate) dirty_flag: Option<Rc<Cell<bool>>>,
}

impl LazyListController {
    pub fn new() -> Self {
        Self {
            offset: 0.0,
            max_scroll_extent: 0.0,
            viewport_dimension: 0.0,
            on_scroll: None,
            on_visible_range_change: None,
            handle: None,
            element_tree: None,
            mutation_queue: None,
            dirty_flag: None,
        }
    }

    fn node_id(&self) -> Option<NodeId> {
        let handle_obj = self.handle.as_ref()?;
        let handle = BoaOpaque::<TurNodeHandle>::wrap(handle_obj)?;
        Some(handle.id)
    }
}

impl Default for LazyListController {
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

impl Class for LazyListController {
    const NAME: &'static str = "LazyListController";
    const LENGTH: usize = 1;

    fn data_constructor(
        _new_target: &JsValue,
        args: &[JsValue],
        ctx: &mut Context,
    ) -> JsResult<Self> {
        let mut ctrl = Self::new();
        if let Some(opts) = args.get_or_undefined(0).as_object() {
            ctrl.on_scroll = extract_mutation_from_opts(&opts, "onScroll", ctx);
            ctrl.on_visible_range_change =
                extract_mutation_from_opts(&opts, "onVisibleRangeChange", ctx);
        }
        Ok(ctrl)
    }

    fn init(class: &mut ClassBuilder<'_>) -> JsResult<()> {
        controller_getter!(class, "offset", |this, _, _| {
            let obj = this.as_object().ok_or_else(|| {
                JsNativeError::typ().with_message("invalid this")
            })?;
            let ctrl = obj
                .downcast_ref::<LazyListController>()
                .ok_or_else(|| JsNativeError::typ().with_message("invalid this"))?;
            Ok(JsValue::from(ctrl.offset))
        });

        controller_getter!(class, "maxScrollExtent", |this, _, _| {
            let obj = this.as_object().ok_or_else(|| {
                JsNativeError::typ().with_message("invalid this")
            })?;
            let ctrl = obj
                .downcast_ref::<LazyListController>()
                .ok_or_else(|| JsNativeError::typ().with_message("invalid this"))?;
            Ok(JsValue::from(ctrl.max_scroll_extent))
        });

        controller_getter!(class, "viewportDimension", |this, _, _| {
            let obj = this.as_object().ok_or_else(|| {
                JsNativeError::typ().with_message("invalid this")
            })?;
            let ctrl = obj
                .downcast_ref::<LazyListController>()
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
                    .downcast_mut::<LazyListController>()
                    .ok_or_else(|| JsNativeError::typ().with_message("invalid this"))?;

                let target_offset = args.get_or_undefined(0).as_number().unwrap_or(0.0);

                let element_tree_rc = ctrl.element_tree.clone();
                let dirty_flag = ctrl.dirty_flag.clone();
                let mutation_queue = ctrl.mutation_queue.clone();
                let node_id = ctrl.node_id();
                let on_scroll = ctrl.on_scroll;
                let on_visible_range_change = ctrl.on_visible_range_change;

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
                let Some(node) = tree.get_element_mut(ElementNodeId::new(node_id.as_u64())) else {
                    return Ok(JsValue::undefined());
                };
                let Some(ref mut element) = node.element else {
                    return Ok(JsValue::undefined());
                };
                let Some(ll) = element.cast_mut::<LazyListElement>() else {
                    return Ok(JsValue::undefined());
                };

                let max = ll.position.max_scroll_extent();
                let clamped = target_offset.clamp(0.0, max);
                ll.position.correct_pixels(clamped);

                let vp = ll.position.viewport_size();
                let dim = match ll.axis {
                    tur_shared::Axis::Vertical => vp.height,
                    tur_shared::Axis::Horizontal => vp.width,
                };

                let viewport_main = tur_shared::Axis::main(&ll.axis, vp);
                let (start, end) = ll.compute_visible_range(viewport_main);

                let new_offset = ll.position.pixels();
                tree.mark_dirty(node_id);
                drop(tree);

                ctrl.offset = new_offset;
                ctrl.max_scroll_extent = max;
                ctrl.viewport_dimension = dim;
                dirty_flag.set(true);

                if let Some(queue_rc) = mutation_queue {
                    if let Some(m) = on_visible_range_change {
                        queue_rc.borrow_mut().push(
                            m,
                            VisibleRangeChangeEvent {
                                start_index: start,
                                end_index: end,
                            },
                        );
                    }
                    if let Some(m) = on_scroll {
                        queue_rc.borrow_mut().push(
                            m,
                            ScrollEvent {
                                offset: ctrl.offset,
                                max_extent: ctrl.max_scroll_extent,
                                viewport_dimension: ctrl.viewport_dimension,
                            },
                        );
                    }
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
                    .downcast_mut::<LazyListController>()
                    .ok_or_else(|| JsNativeError::typ().with_message("invalid this"))?;

                if let Some(handle_obj) = args.get_or_undefined(0).as_object() {
                    if BoaOpaque::<TurNodeHandle>::wrap(&handle_obj).is_some() {
                        ctrl.handle = Some(handle_obj.clone());
                    }
                }

                if let Some(ctx_obj) = args.get_or_undefined(1).as_object() {
                    if let Some(js_ctx) =
                        BoaOpaque::<TurJsContext>::wrap(&ctx_obj)
                    {
                        ctrl.element_tree = Some(js_ctx.element_tree.clone());
                        ctrl.mutation_queue = Some(js_ctx.mutation_queue.clone());
                        ctrl.dirty_flag = Some(js_ctx.dirty.clone());
                    }
                }

                Ok(JsValue::undefined())
            }),
        );

        Ok(())
    }
}
