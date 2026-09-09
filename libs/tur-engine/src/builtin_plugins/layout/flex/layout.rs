use crate::core::layout::{
    Axis, Constraints, CrossAxisAlignment, FlexFit, MainAxisAlignment, MainAxisSize, Offset, Size,
};

use crate::core::element::ElementNodeId;
use crate::core::layout::{ElementLayout, LayoutContext};

use crate::builtin_plugins::layout::flex_item::FlexibleElement;

use super::element::{ChildData, FlexElement};

/// Resolve the `flex` weight of a flex-item child (`Expanded().flex(…)` /
/// `Flexible().flex(…)`). Returns 0.0 if the child is not a flex item. If it
/// is one but the `flex` prop is absent, returns 1.0 (Flutter default).
///
/// This is a flex-plugin helper rather than a `LayoutContext` method:
/// `LayoutContext` is generic infra and shouldn't know about
/// `FlexibleElement` specifically. The generic primitive
/// `LayoutContext::child_element::<T>()` is the only thing this needs from
/// the engine.
fn child_flex(cx: &mut LayoutContext, child_id: ElementNodeId) -> f64 {
    let Some(flex_item) = cx.child_element::<FlexibleElement>(child_id) else {
        return 0.0;
    };
    let Some(flex_val) = flex_item.view.flex.clone() else {
        return 1.0;
    };
    cx.read_val(&flex_val).unwrap_or(1.0).max(0.0)
}

/// Resolve how a flex-item child is inscribed into its slot (Flutter
/// `FlexFit`): `Tight` (`Expanded`) forces the child to fill the slot;
/// `Loose` (`Flexible`) caps it at the slot but allows smaller. Defaults to
/// `Tight` (Expanded semantics — the historical behavior) if the element
/// can't be read.
fn child_fit(cx: &mut LayoutContext, child_id: ElementNodeId) -> FlexFit {
    cx.child_element::<FlexibleElement>(child_id)
        .map(|flex_item| flex_item.view.fit)
        .unwrap_or(FlexFit::Tight)
}

/// Cross-axis `Stretch` tightens children to the incoming cross max. Under an
/// UNBOUNDED cross max (e.g. a Stretch Row nested in a Column) that would
/// mint `min = max = ∞` constraints and leak infinite sizes upward — Flutter
/// errors here; we degrade to a loose cross axis instead.
fn stretch_min(cross_alignment: CrossAxisAlignment, max: f64) -> f64 {
    if cross_alignment == CrossAxisAlignment::Stretch && max.is_finite() {
        max
    } else {
        0.0
    }
}

impl ElementLayout for FlexElement {
    fn perform_layout(
        &mut self,
        constraints: &Constraints,
        children: &[ElementNodeId],
        cx: &mut LayoutContext,
    ) -> Size {
        let direction = self.view.direction.unwrap_or(Axis::Vertical);
        let cross_alignment = cx
            .read_val_opt(self.view.cross_alignment.as_ref())
            .unwrap_or(CrossAxisAlignment::Center);
        let main_axis_size = cx
            .read_val_opt(self.view.main_axis_size.as_ref())
            .unwrap_or(MainAxisSize::Max);

        self.child_data.clear();
        self.constraints = Some(*constraints);

        // Degenerate-case diagnostics (Flutter throws layout errors here; we
        // degrade gracefully but log once per element instance).
        let cross_max = match direction {
            Axis::Vertical => constraints.max_width,
            Axis::Horizontal => constraints.max_height,
        };
        if cross_alignment == CrossAxisAlignment::Stretch
            && !cross_max.is_finite()
            && !self.warned_stretch_unbounded
        {
            self.warned_stretch_unbounded = true;
            tracing::error!(
                "{} with crossAlignment=Stretch has an unbounded cross axis: \
                 Stretch has no effect (children fall back to loose cross \
                 constraints). Give the parent a bounded size (e.g. wrap in \
                 Expanded) — Flutter reports this as a layout error.",
                match direction {
                    Axis::Vertical => "Column",
                    Axis::Horizontal => "Row",
                }
            );
        }

        let mut total_main: f64 = 0.0;
        let mut max_cross: f64 = 0.0;
        let mut total_flex: f64 = 0.0;

        for &child_id in children {
            let is_flex = cx.child_type_name(child_id) == "tur_flex_item";

            if is_flex {
                let flex = child_flex(cx, child_id).max(0.0);
                total_flex += flex;
                self.child_data.push(ChildData {
                    id: child_id,
                    size: Size::ZERO,
                    is_flex: true,
                    flex,
                });
            } else {
                // Flutter RenderFlex parity (`_constraintsForNonFlexChild`):
                // non-flex children get the CROSS axis constraint (tight max
                // under `Stretch`, loose otherwise) but an UNBOUNDED main
                // axis — "Layout each child with a null or zero flex factor
                // with unbounded main axis constraints and the incoming
                // cross axis constraints" (RenderFlex docs, step 1). A
                // nested flex's `MainAxisSize.max` then degenerates to
                // content size under infinite main constraints, so nested
                // Columns/Rows shrink-wrap instead of consuming the parent's
                // extent — and a Text that should ellipsize needs an
                // `Expanded`/`Flexible` wrapper to receive a finite budget,
                // exactly as in Flutter.
                let child_constraints = match direction {
                    Axis::Vertical => Constraints {
                        min_width: stretch_min(cross_alignment, constraints.max_width),
                        max_width: constraints.max_width,
                        min_height: 0.0,
                        max_height: f64::INFINITY,
                    },
                    Axis::Horizontal => Constraints {
                        min_width: 0.0,
                        max_width: f64::INFINITY,
                        min_height: stretch_min(cross_alignment, constraints.max_height),
                        max_height: constraints.max_height,
                    },
                };
                let size = cx.layout_child(child_id, &child_constraints);
                total_main += direction.main(size);
                max_cross = max_cross.max(direction.cross(size));
                self.child_data.push(ChildData {
                    id: child_id,
                    size,
                    is_flex: false,
                    flex: 0.0,
                });
            }
        }

        let available_main = direction
            .main(constraints.constrain(Size::new(constraints.max_width, constraints.max_height)));
        // Flex children under an unbounded main axis have no space to divide
        // (Flutter: "RenderFlex children have non-zero flex but incoming
        // constraints are unbounded"). Degrade to zero slots instead of
        // leaking infinite sizes, and say so once.
        if !available_main.is_finite() && total_flex > 0.0 && !self.warned_flex_unbounded {
            self.warned_flex_unbounded = true;
            tracing::error!(
                "{} has flex (Expanded) children but unbounded main-axis \
                 constraints: flex children collapse to zero size. Wrap it \
                 in a bounded parent (e.g. Expanded outside a ScrollView's \
                 scroll axis).",
                match direction {
                    Axis::Vertical => "Column",
                    Axis::Horizontal => "Row",
                }
            );
        }
        let remaining_main = if available_main.is_finite() {
            (available_main - total_main).max(0.0)
        } else {
            0.0
        };
        let space_per_unit = if total_flex > 0.0 {
            remaining_main / total_flex
        } else {
            0.0
        };

        for entry in &mut self.child_data {
            if entry.is_flex {
                let slot = space_per_unit * entry.flex;
                // Flutter `_constraintsForFlexChild` parity: `FlexFit.tight`
                // (Expanded) tightens the child to the slot; `FlexFit.loose`
                // (Flexible) caps it at the slot with min 0 — the child may
                // be smaller (shrink-wraps), e.g. a label that ellipsizes at
                // the true available width without being force-filled.
                let fit = child_fit(cx, entry.id);
                let min_main = if fit == FlexFit::Tight { slot } else { 0.0 };
                let child_constraints = match direction {
                    Axis::Vertical => Constraints {
                        min_width: stretch_min(cross_alignment, constraints.max_width),
                        max_width: constraints.max_width,
                        min_height: min_main,
                        max_height: slot,
                    },
                    Axis::Horizontal => Constraints {
                        min_width: min_main,
                        max_width: slot,
                        min_height: stretch_min(cross_alignment, constraints.max_height),
                        max_height: constraints.max_height,
                    },
                };
                let size = cx.layout_child(entry.id, &child_constraints);
                entry.size = size;
                max_cross = max_cross.max(direction.cross(size));
            }
        }

        let total_main: f64 = self.child_data.iter().map(|d| direction.main(d.size)).sum();

        let main_size = match main_axis_size {
            MainAxisSize::Max => {
                let max_main = match direction {
                    Axis::Vertical => constraints.max_height,
                    Axis::Horizontal => constraints.max_width,
                };
                if max_main.is_finite() {
                    max_main
                } else {
                    total_main
                }
            }
            MainAxisSize::Min => total_main,
        };

        let size = match direction {
            Axis::Vertical => Size::new(
                max_cross.clamp(constraints.min_width, constraints.max_width),
                main_size.clamp(constraints.min_height, constraints.max_height),
            ),
            Axis::Horizontal => Size::new(
                main_size.clamp(constraints.min_width, constraints.max_width),
                max_cross.clamp(constraints.min_height, constraints.max_height),
            ),
        };

        let final_size = constraints.constrain(size);
        self.computed_size = Some(final_size);

        let allocated_main: f64 = self.child_data.iter().map(|d| direction.main(d.size)).sum();
        self.overflow = (allocated_main - direction.main(final_size)).max(0.0);

        // Assign child offsets now that all sizes are known.
        self.assign_positions(cx);

        final_size
    }
}

impl FlexElement {
    fn assign_positions(&mut self, cx: &mut LayoutContext) {
        if self.child_data.is_empty() {
            return;
        }

        let direction = self.view.direction.unwrap_or(Axis::Vertical);
        let main_alignment = cx
            .read_val_opt(self.view.main_alignment.as_ref())
            .unwrap_or(MainAxisAlignment::Start);
        let cross_alignment = cx
            .read_val_opt(self.view.cross_alignment.as_ref())
            .unwrap_or(CrossAxisAlignment::Center);

        let allocated_main: f64 = self.child_data.iter().map(|d| direction.main(d.size)).sum();

        let container_size = self.computed_size.unwrap_or_else(|| {
            let constraints = self.constraints.unwrap_or(Constraints::NONE);
            constraints.constrain(Size::new(constraints.max_width, constraints.max_height))
        });
        let available_main = direction.main(container_size);

        let mut current_main = match main_alignment {
            MainAxisAlignment::Start | MainAxisAlignment::SpaceBetween => 0.0,
            MainAxisAlignment::Center => (available_main - allocated_main) / 2.0,
            MainAxisAlignment::End => available_main - allocated_main,
            MainAxisAlignment::SpaceAround => {
                (available_main - allocated_main) / (self.child_data.len() as f64 * 2.0)
            }
            MainAxisAlignment::SpaceEvenly => {
                (available_main - allocated_main) / ((self.child_data.len() + 1) as f64)
            }
        };

        let gap = match main_alignment {
            MainAxisAlignment::SpaceBetween if self.child_data.len() > 1 => {
                (available_main - allocated_main) / ((self.child_data.len() - 1) as f64)
            }
            MainAxisAlignment::SpaceAround => {
                (available_main - allocated_main) / (self.child_data.len() as f64 * 2.0) * 2.0
            }
            MainAxisAlignment::SpaceEvenly => {
                (available_main - allocated_main) / ((self.child_data.len() + 1) as f64)
            }
            _ => 0.0,
        };

        let container_cross = direction.cross(container_size);

        for (i, entry) in self.child_data.iter().enumerate() {
            let cross = match cross_alignment {
                CrossAxisAlignment::Start => 0.0,
                CrossAxisAlignment::Center => (container_cross - direction.cross(entry.size)) / 2.0,
                CrossAxisAlignment::End => container_cross - direction.cross(entry.size),
                CrossAxisAlignment::Stretch => 0.0,
            };

            let offset = match direction {
                Axis::Vertical => Offset::new(cross, current_main),
                Axis::Horizontal => Offset::new(current_main, cross),
            };

            cx.set_child_offset(entry.id, offset);

            current_main += direction.main(entry.size);
            if i < self.child_data.len() - 1 {
                current_main += gap;
            }
        }
    }
}
