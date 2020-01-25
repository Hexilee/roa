use http::StatusCode;
use std::fmt::{Display, Formatter};

#[derive(Debug)]
pub struct Status {
    pub status_code: StatusCode,
    pub kind: StatusKind,

    // response body if self.expose
    pub message: String,

    // if message exposed
    pub expose: bool,
}

#[derive(Debug, Eq, PartialEq)]
pub enum StatusKind {
    /// [[RFC7231, Section 6.2](https://tools.ietf.org/html/rfc7231#section-6.2)]
    Informational,

    /// [[RFC7231, Section 6.3](https://tools.ietf.org/html/rfc7231#section-6.3)]
    Successful,

    /// [[RFC7231, Section 6.4](https://tools.ietf.org/html/rfc7231#section-6.4)]
    Redirection,

    /// [[RFC7231, Section 6.5](https://tools.ietf.org/html/rfc7231#section-6.5)]
    ClientError,

    /// [[RFC7231, Section 6.6](https://tools.ietf.org/html/rfc7231#section-6.6)]
    ServerError,

    Unknown,
}

impl StatusKind {
    fn infer(status_code: StatusCode) -> Self {
        use StatusKind::*;
        match status_code.as_u16() / 100 {
            1 => Informational,
            2 => Successful,
            3 => Redirection,
            4 => ClientError,
            5 => ServerError,
            _ => Unknown,
        }
    }
}

impl Status {
    pub fn new(status_code: StatusCode, message: String, expose: bool) -> Self {
        Self {
            status_code,
            kind: StatusKind::infer(status_code),
            message,
            expose,
        }
    }

    pub(crate) fn need_throw(&self) -> bool {
        self.kind == StatusKind::ServerError || self.kind == StatusKind::Unknown
    }
}

impl From<std::io::Error> for Status {
    fn from(err: std::io::Error) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, err.to_string(), false)
    }
}

impl From<http::Error> for Status {
    fn from(err: http::Error) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, err.to_string(), false)
    }
}

impl Display for Status {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        f.write_str(&format!("{}: {}", self.status_code, self.message))
    }
}

impl std::error::Error for Status {}
