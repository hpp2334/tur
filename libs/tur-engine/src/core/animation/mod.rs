use std::collections::HashMap;

use boa_engine::object::JsObject;
use tur_shared::{AnimatableValue, AnimationCurve, TransitionConfig, Tween};

use crate::core::element::ElementNodeId;
use crate::core::elements::ElementTree;

pub mod controller;
pub use controller::AnimationController;

pub struct AnimationTickResult {
    pub element_id: ElementNodeId,
    pub property: String,
    pub value: AnimatableValue,
    pub affects_layout: bool,
}

#[derive(Debug)]
struct ImplicitAnimation {
    element_id: ElementNodeId,
    property: String,
    tween: Tween,
    curve: AnimationCurve,
    duration_ms: u64,
    start_time_ms: u64,
}

#[derive(Debug, Default)]
pub struct AnimationManager {
    implicit_animations: Vec<ImplicitAnimation>,
    transitions: HashMap<ElementNodeId, HashMap<String, TransitionConfig>>,
    controllers: Vec<JsObject>,
}

impl AnimationManager {
    pub fn new() -> Self {
        AnimationManager {
            implicit_animations: Vec::new(),
            transitions: HashMap::new(),
            controllers: Vec::new(),
        }
    }

    pub fn register_controller(&mut self, obj: JsObject) {
        if !self.controllers.iter().any(|c| c == &obj) {
            self.controllers.push(obj);
        }
    }

    pub fn tick_controllers(&mut self, now_ms: u64) {
        let mut active = Vec::new();
        for obj in self.controllers.drain(..) {
            let keep = {
                let Some(mut ctrl) = obj.downcast_mut::<AnimationController>() else {
                    continue;
                };
                let was_active = ctrl.is_active();
                if was_active {
                    ctrl.tick(now_ms);
                }
                ctrl.is_active()
            };
            if keep {
                active.push(obj);
            }
        }
        self.controllers = active;
    }

    pub fn set_transitions(
        &mut self,
        id: ElementNodeId,
        transitions: HashMap<String, TransitionConfig>,
    ) {
        self.transitions.insert(id, transitions);
    }

    pub fn remove_transitions(&mut self, id: ElementNodeId) {
        self.transitions.remove(&id);
        self.implicit_animations
            .retain(|a| a.element_id != id);
    }

    pub fn get_transition(
        &self,
        id: ElementNodeId,
        property: &str,
    ) -> Option<&TransitionConfig> {
        self.transitions.get(&id).and_then(|m| m.get(property))
    }

    pub fn start_implicit(
        &mut self,
        id: ElementNodeId,
        property: &str,
        from: AnimatableValue,
        to: AnimatableValue,
        config: &TransitionConfig,
        now_ms: u64,
    ) {
        self.implicit_animations
            .retain(|a| !(a.element_id == id && a.property == property));

        let tween = match (from, to) {
            (AnimatableValue::Float(b), AnimatableValue::Float(e)) => {
                Tween::Float { begin: b, end: e }
            }
            (AnimatableValue::Color(b), AnimatableValue::Color(e)) => {
                Tween::Color { begin: b, end: e }
            }
            (AnimatableValue::Size(b), AnimatableValue::Size(e)) => {
                Tween::Size { begin: b, end: e }
            }
            (AnimatableValue::Offset(b), AnimatableValue::Offset(e)) => {
                Tween::Offset { begin: b, end: e }
            }
            _ => return,
        };

        self.implicit_animations.push(ImplicitAnimation {
            element_id: id,
            property: property.to_string(),
            tween,
            curve: config.curve,
            duration_ms: config.duration_ms,
            start_time_ms: now_ms,
        });
    }

    pub fn tick(&mut self, now_ms: u64) -> Vec<AnimationTickResult> {
        let mut results = Vec::new();
        let mut remaining = Vec::new();

        for anim in self.implicit_animations.drain(..) {
            let elapsed = now_ms.saturating_sub(anim.start_time_ms);
            let t = (elapsed as f64 / anim.duration_ms as f64).min(1.0);
            let value = anim.tween.evaluate(t, &anim.curve);

            results.push(AnimationTickResult {
                element_id: anim.element_id,
                property: anim.property.clone(),
                value,
                affects_layout: Self::property_affects_layout(&anim.property),
            });

            if t < 1.0 {
                remaining.push(anim);
            }
        }
        self.implicit_animations = remaining;
        results
    }

    pub fn apply_tick_results(results: &[AnimationTickResult], tree: &mut ElementTree) {
        for result in results {
            if let Some(node) = tree.get_mut(result.element_id) {
                if let Some(ref mut element) = node.element {
                    element.apply_animated(&result.property, result.value.clone());
                }
            }
            if result.affects_layout {
                tree.mark_dirty(result.element_id);
            } else {
                tree.mark_dirty_paint(result.element_id);
            }
        }
    }

    pub fn has_active(&self) -> bool {
        !self.implicit_animations.is_empty() || !self.controllers.is_empty()
    }

    pub fn property_affects_layout(property: &str) -> bool {
        matches!(
            property,
            "width"
                | "height"
                | "padding"
                | "borderWidth"
                | "left"
                | "top"
                | "right"
                | "bottom"
                | "flex"
        )
    }
}
