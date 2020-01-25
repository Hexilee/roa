use http::StatusCode;
use std::fmt::{Display, Formatter};

#[derive(Debug)]
pub struct Error {
    pub status_code: StatusCode,
    pub kind: ErrorKind,
    pub cause: Box<dyn std::error::Error + Send + Sync>,
}

#[derive(Debug, Eq, PartialEq)]
pub enum ErrorKind {
    /// [[RFC7231, Section 6.2.1](https://tools.ietf.org/html/rfc7231#section-6.2)]
    Informational,

    /// [[RFC7231, Section 6.2.1](https://tools.ietf.org/html/rfc7231#section-6.4)]
    Redirection,

    /// [[RFC7231, Section 6.2.1](https://tools.ietf.org/html/rfc7231#section-6.5)]
    ClientError,

    /// [[RFC7231, Section 6.2.1](https://tools.ietf.org/html/rfc7231#section-6.6)]
    ServerError,
}

impl Error {
    pub fn new(
        status_code: StatusCode,
        kind: ErrorKind,
        cause: impl Into<Box<dyn std::error::Error + Send + Sync>>,
    ) -> Self {
        Self {
            status_code,
            kind,
            cause: cause.into(),
        }
    }
}

impl From<hyper::Error> for Error {
    fn from(err: hyper::Error) -> Self {
        let (status_code, kind) = if err.is_parse() || err.is_incomplete_message() {
            (StatusCode::BAD_REQUEST, ErrorKind::ClientError)
        } else {
            (StatusCode::INTERNAL_SERVER_ERROR, ErrorKind::ServerError)
        };
        Self {
            status_code,
            kind,
            cause: Box::new(err),
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Self {
            status_code: StatusCode::INTERNAL_SERVER_ERROR,
            kind: ErrorKind::ServerError,
            cause: Box::new(err),
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        f.write_str(&format!("{}: {}", self.status_code, self.cause))
    }
}

impl std::error::Error for Error {}
