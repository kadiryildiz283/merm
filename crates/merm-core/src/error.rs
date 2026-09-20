use thiserror::Error;

#[derive(Error, Debug, PartialEq, Eq)]
pub enum CoreError {
    #[error("No diagram found in input")]
    NoDiagramFound,

    #[error("Execution timed out after {0:?}. Diagram layout calculation exceeded execution budget.")]
    ExecutionTimeout(std::time::Duration),

    #[error("Memory quota exceeded: {0} bytes allocated")]
    MemoryQuotaExceeded(usize),

    #[error("Syntax error in diagram: {0}")]
    SyntaxError(String),

    #[error("Unsupported diagram type: {0}")]
    UnsupportedType(String),
}
