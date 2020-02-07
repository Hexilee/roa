pub use http::StatusCode;
use std::fmt::{Display, Formatter};
use std::future::Future;
use std::pin::Pin;
use std::result::Result as StdResult;

pub type Result<R = ()> = StdResult<R, Error>;

pub type ResultFuture<R = ()> = Pin<Box<dyn 'static + Future<Output = Result<R>> + Send>>;

pub fn throw<R>(status_code: StatusCode, message: impl ToString) -> Result<R> {
    Err(Error::new(status_code, message, true))
}

#[derive(Debug, Clone)]
pub struct Error {
    pub status_code: StatusCode,

    pub kind: ErrorKind,

    // response body if self.expose
    pub message: String,

    // if message exposed
    pub expose: bool,
}

#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub enum ErrorKind {
    /// [[RFC7231, Section 6.2](https://tools.ietf.org/html/rfc7231#section-6.2)]
    Informational,

    /// [[RFC7231, Section 6.3](https://tools.ietf.org/html/rfc7231#section-6.3)]
    // Successful,

    /// [[RFC7231, Section 6.4](https://tools.ietf.org/html/rfc7231#section-6.4)]
    Redirection,

    /// [[RFC7231, Section 6.5](https://tools.ietf.org/html/rfc7231#section-6.5)]
    ClientError,

    /// [[RFC7231, Section 6.6](https://tools.ietf.org/html/rfc7231#section-6.6)]
    ServerError,

    Unknown,
}

impl ErrorKind {
    fn infer(status_code: StatusCode) -> Self {
        use ErrorKind::*;
        match status_code.as_u16() / 100 {
            1 => Informational,
            2 => panic!(
                r"2xx status cannot be thrown.
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

impl Error {
    pub fn new(status_code: StatusCode, message: impl ToString, expose: bool) -> Self {
        Self {
            status_code,
            kind: ErrorKind::infer(status_code),
            message: message.to_string(),
            expose,
        }
    }

    pub(crate) fn need_throw(&self) -> bool {
        self.kind == ErrorKind::ServerError || self.kind == ErrorKind::Unknown
    }
}

macro_rules! internal_server_error {
    ($error:ty) => {
        impl From<$error> for Error {
            fn from(err: $error) -> Self {
                Self::new(StatusCode::INTERNAL_SERVER_ERROR, err, false)
            }
        }
    };
}

internal_server_error!(std::io::Error);
internal_server_error!(http::Error);

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> StdResult<(), std::fmt::Error> {
        f.write_str(&format!("{}: {}", self.status_code, self.message))
    }
}

impl std::error::Error for Error {}
