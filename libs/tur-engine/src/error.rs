use boa_engine::JsError;

#[derive(Debug, thiserror::Error)]
pub enum TurError {
    #[error("JS evaluation failed: {0}")]
    JsEval(#[source] JsError),
    #[error("IO error: {0}")]
    Io(#[source] std::io::Error),
}
