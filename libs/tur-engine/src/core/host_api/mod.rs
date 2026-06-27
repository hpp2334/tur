use tur_shared::Cursor;

/// Host-facing API the engine pushes shell outputs through.
///
/// Implemented by the embedder: `tur-wasm` applies the cursor to the host
/// canvas; tests record it for assertions. The engine owns a `Box<dyn HostApi>`
/// inside [`crate::core::shell::ShellInternal`] and calls these methods from
/// the driver (e.g. `apply_changes` pushes the resolved cursor). This replaces
/// the old poll model (`take_current_cursor`): the engine now *pushes* shell
/// outputs rather than the embedder *polling* them.
pub trait HostApi {
    /// Apply the resolved host cursor. Called by the driver only when the
    /// cursor actually changes.
    fn set_cursor(&mut self, cursor: Cursor);
}

/// Convenience `HostApi` that discards everything. Used by embedders/tests
/// that don't care about shell outputs (e.g. the vello render-to-pixel tests).
#[derive(Default)]
pub struct NoopHostApi;

impl HostApi for NoopHostApi {
    fn set_cursor(&mut self, _cursor: Cursor) {}
}
