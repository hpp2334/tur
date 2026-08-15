//! Opaque render-surface plumbing.
//!
//! The engine core knows nothing about platforms: a **surface** is an opaque,
//! embedder-supplied value that only the paired [`Renderer`]
//! implementation understands. Concrete surface payloads (a browser canvas, a
//! raw window/display handle pair, the unit `NoopSurface`, …) live next to
//! their renderer impls in `renderer/vello` / `renderer/noop` — never in
//! `core/`.
//!
//! A [`Renderer`] is a **factory** holding the shared backend substrate (e.g.
//! one wgpu instance); each [`crate::core::render::RenderTarget`] it creates
//! from a surface owns exactly one render target (canvas / window surface).

/// Marker trait for embedder-supplied render-surface payloads.
///
/// Implemented by concrete surface types co-located with their renderer
/// implementations (`RawSurface`, `CanvasSurface`, `NoopSurface`). The engine
/// only ever sees the boxed form ([`SurfaceHandle`]); the renderer
/// downcasts and errors clearly on a mismatched pairing.
pub trait Surface: std::any::Any + 'static {
    /// Upcast to `Box<dyn Any>` so the paired renderer can downcast to its
    /// concrete surface type by value (`Box<dyn Surface>` cannot be
    /// directly downcast). Implementors write `self`.
    fn into_any(self: Box<Self>) -> Box<dyn std::any::Any>;
}

/// Owned, type-erased surface passed to
/// [`Renderer::create_target`](crate::core::render::Renderer::create_target)
/// via [`TurApp::setup_root`](crate::TurApp::setup_root).
pub type SurfaceHandle = Box<dyn Surface>;
