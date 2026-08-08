use boa_engine::JsError;

#[derive(Debug, thiserror::Error)]
pub enum TurError {
    #[error("JS evaluation failed: {0}")]
    JsEval(#[source] JsError),
    #[error("IO error: {0}")]
    Io(#[source] std::io::Error),
    #[error("render failed: {0}")]
    Render(String),
    #[error("{0}")]
    Other(String),
}

impl From<crate::core::app::ModuleError> for TurError {
    fn from(e: crate::core::app::ModuleError) -> Self {
        match e {
            crate::core::app::ModuleError::Parse(msg) => {
                TurError::Other(format!("module parse: {msg}"))
            }
            crate::core::app::ModuleError::Eval(msg) => {
                TurError::Other(format!("module eval: {msg}"))
            }
            crate::core::app::ModuleError::WorkerGone => TurError::Other("worker gone".into()),
        }
    }
}
