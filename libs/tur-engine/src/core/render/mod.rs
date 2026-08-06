mod canvas;
mod command;
mod element_render;
mod paint_context;
mod playback;
mod recording_canvas;
mod renderer;

pub mod brush;

pub use canvas::*;
pub use command::{CanvasOp, RenderCommand};
pub use element_render::*;
pub use paint_context::*;
pub use playback::play_commands;
pub use recording_canvas::RecordingCanvas;
pub use renderer::*;

/// One frame's worth of paint state shipped worker → main
/// (`MainMsg::RenderCommands`). Plain `Vec<RenderCommand>` moved by
/// ownership across the channel (no clone).
pub type RenderCommandBatch = Vec<RenderCommand>;
