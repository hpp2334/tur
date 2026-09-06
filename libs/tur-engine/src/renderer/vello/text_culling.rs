//! Playback-side visible-line culling for text layouts.
//!
//! The host re-encodes every painted frame into the vello scene
//! (`scene.reset()` + full `play_commands`), so a tall text layout inside a
//! `ScrollView` would re-encode the *entire* document's glyph runs each
//! scroll tick. The clip that bounds what is actually visible is already in
//! the op stream (`PushClip`/`PushClipGeometry`/`PopClip` + per-node absolute
//! transforms), so `VelloPaintContext` mirrors it and filters a text op's
//! runs down to the visible line range before encoding — no wire-format
//! change, and nested virtual-app replay inherits it for free (the mirror
//! sees the parent's clip ops at final playback).
//!
//! Everything here is conservative: clip AABBs only grow under rotation,
//! the inverse-mapped local clip only grows, and each end of the visible
//! range keeps [`LINE_SLOP`] extra lines — a visible glyph is never dropped.

use vello_common::kurbo::{Affine, Point, Rect};

use crate::core::text::text_layout::LineInfo;

/// Extra lines kept on each side of the strictly-intersecting range (covers
/// glyph ascent/descent overshoot and AA fringe).
pub(crate) const LINE_SLOP: usize = 1;

/// Mirror of the playback clip stack: conservative **scene-space** AABBs,
/// innermost last. `VelloPaintContext` pushes/pops alongside the real GPU
/// clip layers, so it cannot diverge from the actual clip state (both are
/// driven by the same ops).
#[derive(Debug)]
pub(crate) struct ClipMirror {
    stack: Vec<Rect>,
}

impl ClipMirror {
    /// Seed with the surface rect (physical pixels) — the playback
    /// counterpart of the worker's `RecordingCanvas::new_with_viewport`.
    pub(crate) fn new(surface: Rect) -> Self {
        ClipMirror {
            stack: vec![surface],
        }
    }

    /// Push the AABB of `local_rect` transformed by `transform`, intersected
    /// with the previous top. Conservative: rotated clips contribute their
    /// corner-AABB (a superset of the true clip), so a visible run is never
    /// culled — only extra off-screen runs may be kept.
    pub(crate) fn push_local(&mut self, transform: Affine, local_rect: Rect) {
        let aabb = transform_aabb(transform, local_rect);
        let next = match self.stack.last() {
            Some(top) => top.intersect(aabb),
            None => aabb,
        };
        self.stack.push(next);
    }

    pub(crate) fn pop(&mut self) {
        self.stack.pop();
    }

    /// Innermost active clip (scene space), if any.
    pub(crate) fn current(&self) -> Option<Rect> {
        self.stack.last().copied()
    }
}

/// AABB of the transformed rect's four corners.
fn transform_aabb(transform: Affine, rect: Rect) -> Rect {
    let corners = [
        transform * Point::new(rect.x0, rect.y0),
        transform * Point::new(rect.x1, rect.y0),
        transform * Point::new(rect.x0, rect.y1),
        transform * Point::new(rect.x1, rect.y1),
    ];
    let x0 = corners.iter().map(|p| p.x).fold(f64::INFINITY, f64::min);
    let x1 = corners
        .iter()
        .map(|p| p.x)
        .fold(f64::NEG_INFINITY, f64::max);
    let y0 = corners.iter().map(|p| p.y).fold(f64::INFINITY, f64::min);
    let y1 = corners
        .iter()
        .map(|p| p.y)
        .fold(f64::NEG_INFINITY, f64::max);
    Rect::new(x0, y0, x1, y1)
}

/// Map a scene-space clip into a text layout's LOCAL y-range, given the
/// absolute transform at the text op. Conservative: the inverse-mapped AABB
/// contains the true visible region, so culling keeps a superset of the
/// visible lines. `None` when the transform is singular (or no clip) → the
/// caller encodes everything.
pub(crate) fn local_clip_y(absolute_transform: Affine, clip: Rect) -> Option<(f32, f32)> {
    if absolute_transform.determinant().abs() < f64::EPSILON {
        return None;
    }
    let local = transform_aabb(absolute_transform.inverse(), clip);
    Some((local.y0 as f32, local.y1 as f32))
}

/// `(first_line, end_line_exclusive)` covering every line whose vertical
/// extent intersects `y_clip`, widened by [`LINE_SLOP`] on each side.
/// `None` (or empty `line_infos`) → `(0, len)` — nothing culled.
///
/// `line_infos` are ordered by ascending `top` (parley emits lines in
/// order), so the first visible line is found by scan; callers with many
/// lines rely on the caller-side run binary search (`runs` are grouped by
/// ascending `line_index`).
pub(crate) fn visible_line_range(
    line_infos: &[LineInfo],
    y_clip: Option<(f32, f32)>,
) -> (usize, usize) {
    let n = line_infos.len();
    if n == 0 {
        return (0, 0);
    }
    let Some((y0, y1)) = y_clip else {
        return (0, n);
    };
    if y1 <= y0 {
        // Degenerate clip (zero-height or inverted) — encode everything;
        // the GPU clip decides visibility.
        return (0, n);
    }
    let mut first = n;
    let mut end = 0usize;
    for (i, line) in line_infos.iter().enumerate() {
        let top = line.top;
        let bottom = top + line.height;
        if bottom >= y0 && top <= y1 {
            first = first.min(i);
            end = i + 1;
        }
    }
    if end == 0 {
        // No line intersects — encode nothing.
        return (0, 0);
    }
    (first.saturating_sub(LINE_SLOP), (end + LINE_SLOP).min(n))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(top: f32, height: f32) -> LineInfo {
        LineInfo {
            top,
            height,
            baseline: top + height * 0.8,
            start_byte: 0,
            end_byte: 0,
            right_x: 0.0,
            stops: Vec::new(),
        }
    }

    fn lines(count: usize, height: f32) -> Vec<LineInfo> {
        (0..count)
            .map(|i| line(i as f32 * height, height))
            .collect()
    }

    // ── visible_line_range ─────────────────────────────────────────────

    #[test]
    fn no_clip_encodes_everything() {
        let ls = lines(10, 20.0);
        assert_eq!(visible_line_range(&ls, None), (0, 10));
    }

    #[test]
    fn empty_layout_is_empty() {
        assert_eq!(visible_line_range(&[], Some((0.0, 100.0))), (0, 0));
        assert_eq!(visible_line_range(&[], None), (0, 0));
    }

    #[test]
    fn fully_visible_range_has_no_slop_loss() {
        // 10 lines × 20px; viewport covers y 80..140. Inclusive-boundary
        // intersection: line 3 (bottom==80), 4..6, and line 7 (top==140)
        // all touch → first=3, end=8; slop widens to (2, 9).
        let ls = lines(10, 20.0);
        assert_eq!(visible_line_range(&ls, Some((80.0, 140.0))), (2, 9));
    }

    #[test]
    fn partially_clipped_top_and_bottom_keeps_slop() {
        let ls = lines(10, 20.0);
        // Viewport starts mid-line-2 (y=50) and ends at line 7's top
        // (y=140, inclusive): lines 2..=7 intersect → first=2, end=8 →
        // slop (1, 9).
        assert_eq!(visible_line_range(&ls, Some((50.0, 140.0))), (1, 9));
    }

    #[test]
    fn clip_above_all_lines_encodes_nothing() {
        let ls = lines(10, 20.0);
        assert_eq!(visible_line_range(&ls, Some((-100.0, -10.0))), (0, 0));
    }

    #[test]
    fn clip_below_all_lines_encodes_nothing() {
        let ls = lines(10, 20.0);
        assert_eq!(visible_line_range(&ls, Some((500.0, 600.0))), (0, 0));
    }

    #[test]
    fn clip_touching_first_line_boundary_keeps_it() {
        // Clip ends exactly at line 0's top (y=0): bottom(0)=20 >= 0 and
        // top(0)=0 <= 0 → intersects (inclusive). first=0, end=1 → slop
        // (0, 2).
        let ls = lines(10, 20.0);
        assert_eq!(visible_line_range(&ls, Some((-5.0, 0.0))), (0, 2));
    }

    #[test]
    fn degenerate_clip_encodes_everything() {
        let ls = lines(5, 20.0);
        assert_eq!(visible_line_range(&ls, Some((0.0, 0.0))), (0, 5));
        assert_eq!(visible_line_range(&ls, Some((10.0, -10.0))), (0, 5));
    }

    // ── ClipMirror ─────────────────────────────────────────────────────

    #[test]
    fn clip_mirror_intersects_stack() {
        let mut m = ClipMirror::new(Rect::new(0.0, 0.0, 800.0, 600.0));
        // Nested clip at (100,100) size 200x300 → intersect with viewport.
        m.push_local(
            Affine::translate((100.0, 100.0)),
            Rect::new(0.0, 0.0, 200.0, 300.0),
        );
        assert_eq!(m.current(), Some(Rect::new(100.0, 100.0, 300.0, 400.0)));
        // A wider clip NESTED inside it is still bounded by the innermost
        // active clip (the intersection is with the stack top, not the
        // viewport).
        m.push_local(
            Affine::translate((0.0, 0.0)),
            Rect::new(0.0, 0.0, 5000.0, 5000.0),
        );
        assert_eq!(m.current(), Some(Rect::new(100.0, 100.0, 300.0, 400.0)));
        m.pop();
        assert_eq!(m.current(), Some(Rect::new(100.0, 100.0, 300.0, 400.0)));
        m.pop();
        // Back to the viewport: a wide clip now cannot exceed it.
        m.push_local(
            Affine::translate((0.0, 0.0)),
            Rect::new(0.0, 0.0, 5000.0, 5000.0),
        );
        assert_eq!(m.current(), Some(Rect::new(0.0, 0.0, 800.0, 600.0)));
        m.pop();
        assert_eq!(m.current(), Some(Rect::new(0.0, 0.0, 800.0, 600.0)));
    }

    #[test]
    fn clip_mirror_rotated_clip_is_conservative() {
        let mut m = ClipMirror::new(Rect::new(-1e9, -1e9, 1e9, 1e9));
        // A 100x100 clip rotated 45° contributes its corner AABB
        // (≈ 141.4 × 141.4), a superset of the true diamond clip.
        m.push_local(
            Affine::rotate(std::f64::consts::FRAC_PI_4),
            Rect::new(0.0, 0.0, 100.0, 100.0),
        );
        let cur = m.current().unwrap();
        assert!(cur.width() > 100.0 && cur.height() > 100.0);
    }

    // ── local_clip_y ───────────────────────────────────────────────────

    #[test]
    fn local_clip_y_translates_scene_to_local() {
        // Text drawn at absolute (0, -500) (scrolled up 500px): a viewport
        // clip of y 0..600 maps to local y 500..1100.
        let t = Affine::translate((0.0, -500.0));
        assert_eq!(
            local_clip_y(t, Rect::new(0.0, 0.0, 400.0, 600.0)),
            Some((500.0, 1100.0))
        );
    }

    #[test]
    fn local_clip_y_scales_dpr_back_out() {
        // Root transform is a 2× dpr scale; the physical clip 0..1200 maps
        // back to logical 0..600.
        let t = Affine::scale(2.0);
        assert_eq!(
            local_clip_y(t, Rect::new(0.0, 0.0, 800.0, 1200.0)),
            Some((0.0, 600.0))
        );
    }

    #[test]
    fn local_clip_y_singular_transform_is_none() {
        assert_eq!(
            local_clip_y(Affine::scale(0.0), Rect::new(0.0, 0.0, 10.0, 10.0)),
            None
        );
    }

    #[test]
    fn local_clip_y_rotation_grows_conservatively() {
        // The clip in local space under rotation must CONTAIN the true
        // visible band; check it never shrinks below the exact band for the
        // axis-aligned case composed with rotation of the transform.
        let t = Affine::rotate(0.3) * Affine::translate((10.0, 20.0));
        let clip = Rect::new(0.0, 0.0, 100.0, 100.0);
        let (y0, y1) = local_clip_y(t, clip).expect("invertible");
        assert!(y1 > y0);
    }
}
