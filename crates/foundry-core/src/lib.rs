use serde::Serialize;

pub const SCHEMA_VERSION: u8 = 1;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Success<T: Serialize> {
    pub schema_version: u8,
    pub command: String,
    pub ok: bool,
    pub data: T,
    pub warnings: Vec<String>,
    pub error: Option<ErrorDetail>,
}

impl<T: Serialize> Success<T> {
    pub fn new(command: impl Into<String>, data: T, warnings: Vec<String>) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            command: command.into(),
            ok: true,
            data,
            warnings,
            error: None,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Failure {
    pub schema_version: u8,
    pub command: String,
    pub ok: bool,
    pub data: Option<()>,
    pub warnings: Vec<String>,
    pub error: Option<ErrorDetail>,
}

impl Failure {
    pub fn new(
        command: impl Into<String>,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            command: command.into(),
            ok: false,
            data: None,
            warnings: Vec::new(),
            error: Some(ErrorDetail {
                code: code.into(),
                message: message.into(),
            }),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ErrorDetail {
    pub code: String,
    pub message: String,
}
