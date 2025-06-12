use axoasset::reqwest;
use miette::Diagnostic;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, GazenotError>;
pub type ResultInner<T> = std::result::Result<T, GazenotErrorInner>;

#[derive(Error, Debug, Diagnostic)]
#[error("couldn't {operation}")]
pub struct GazenotError {
    pub operation: String,
    #[help]
    pub help: Option<String>,
    #[source]
    #[diagnostic_source]
    pub cause: GazenotErrorInner,
}

impl GazenotError {
    pub fn new(operation: impl Into<String>, err: impl Into<GazenotErrorInner>) -> Self {
        Self {
            operation: operation.into(),
            help: None,
            cause: err.into(),
        }
    }
    pub fn with_url(
        operation: impl Into<String>,
        url: impl std::fmt::Display,
        err: impl Into<GazenotErrorInner>,
    ) -> Self {
        Self {
            operation: operation.into(),
            help: Some(format!("was accessing this endpoint: {url}")),
            cause: err.into(),
        }
    }
}

#[derive(Error, Debug, Diagnostic)]
#[error("{0}")]
pub struct SimpleError(pub String);

#[derive(Error, Debug, Diagnostic)]
pub enum GazenotErrorInner {
    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),
    #[error(transparent)]
    HeaderValue(#[from] reqwest::header::InvalidHeaderValue),
    #[error(transparent)]
    Join(#[from] tokio::task::JoinError),
    #[error(transparent)]
    Url(#[from] url::ParseError),
    #[error(transparent)]
    Asset(#[from] axoasset::AxoassetError),
    #[error("server error {status}")]
    ResponseError {
        status: reqwest::StatusCode,
        #[related]
        errors: Vec<SimpleError>,
    },
    #[error("failed to load axodotdev api credentials for Abyss: {reason}")]
    #[diagnostic(help("is {env_var_name} properly set?"))]
    AuthKey {
        reason: &'static str,
        env_var_name: &'static str,
    },
    #[error("attempted to access production API with mock hosting info")]
    #[diagnostic(help(
        "did you run 'cargo dist host --steps=create'? (your CI should do this for you)"
    ))]
    IsMocked,
}
