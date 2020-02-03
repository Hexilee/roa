use http::header::{InvalidHeaderName, InvalidHeaderValue, ToStrError};
pub use http::StatusCode;
use std::fmt::{Display, Formatter};
use std::future::Future;
use std::pin::Pin;

pub type StatusFuture<R = ()> = Pin<Box<dyn 'static + Future<Output = Result<R, Status>> + Send>>;

pub fn throw<R>(status_code: StatusCode, message: impl ToString) -> Result<R, Status> {
    Err(Status::new(status_code, message, true))
}

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
    /// Successful,

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
            2 => panic!(
                r"2xx status code cannot be thrown.
                Please use `ctx.resp_mut().await.status = xxx` to set it.
            "
            ),
            3 => Redirection,
            4 => ClientError,
            5 => ServerError,
            _ => Unknown,
        }
    }
}

impl Status {
    pub fn new(status_code: StatusCode, message: impl ToString, expose: bool) -> Self {
        Self {
            status_code,
            kind: StatusKind::infer(status_code),
            message: message.to_string(),
            expose,
        }
    }

    pub(crate) fn need_throw(&self) -> bool {
        self.kind == StatusKind::ServerError || self.kind == StatusKind::Unknown
    }
}

macro_rules! internal_server_error {
    ($error:ty) => {
        impl From<$error> for Status {
            fn from(err: $error) -> Self {
                Self::new(StatusCode::INTERNAL_SERVER_ERROR, err, false)
            }
        }
    };
}

internal_server_error!(std::io::Error);
internal_server_error!(http::Error);
internal_server_error!(InvalidHeaderValue);
internal_server_error!(InvalidHeaderName);
internal_server_error!(ToStrError);

impl Display for Status {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        f.write_str(&format!("{}: {}", self.status_code, self.message))
    }
}

impl std::error::Error for Status {}
