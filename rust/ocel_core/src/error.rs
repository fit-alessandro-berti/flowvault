use std::fmt::Display;

pub type OcelResult<T> = Result<T, OcelError>;

#[derive(Debug, Clone)]
pub struct OcelError {
    message: String,
}

impl OcelError {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl Display for OcelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for OcelError {}
