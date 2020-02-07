pub use http::StatusCode;
use std::fmt::{Display, Formatter};
use std::future::Future;
use std::pin::Pin;
use std::result::Result as StdResult;

/// Type alias for `StdResult<R, Error>`.
pub type Result<R = ()> = StdResult<R, Error>;

/// Type alias for `Pin<Box<dyn 'static + Future<Output = Result<R>> + Send>>`.
pub type ResultFuture<R = ()> = Pin<Box<dyn 'static + Future<Output = Result<R>> + Send>>;

/// Return an exposed Err(Error).
///
/// ### Example
/// ```rust
/// use roa_core::{App, throw};
/// use async_std::task::spawn;
/// use http::{StatusCode};
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let (addr, server) = App::new(())
///         .gate(|ctx, next| async move {
///             next().await?; // throw
///             unreachable!();
///             ctx.resp_mut().await.status = StatusCode::OK;
///             Ok(())
///         })
///         .gate(|_ctx, _next| async {
///             throw(StatusCode::IM_A_TEAPOT, "I'm a teapot!")?; // throw
///             unreachable!()
///         })
///         .run_local()?;
///     spawn(server);
///     let resp = reqwest::get(&format!("http://{}", addr)).await?;
///     assert_eq!(StatusCode::IM_A_TEAPOT, resp.status());
///     assert_eq!("I'm a teapot!", resp.text().await?);
///     Ok(())
/// }
/// ```
pub fn throw<R>(status_code: StatusCode, message: impl ToString) -> Result<R> {
    Err(Error::new(status_code, message, true))
}

/// The `Error` of roa.
#[derive(Debug, Clone)]
pub struct Error {
    /// StatusCode will be responded to client if Error is thrown by the top middleware.
    /// ### Range
    /// 1xx/3xx/4xx/5xx
    ///
    /// ### Example
    /// ```rust
    /// use roa_core::{App, throw};
    /// use async_std::task::spawn;
    /// use http::{StatusCode};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let (addr, server) = App::new(())
    ///         .gate(|ctx, next| async move {
    ///             ctx.resp_mut().await.status = StatusCode::OK;
    ///             next().await // not caught
    ///         })
    ///         .gate(|_ctx, _next| async {
    ///             throw(StatusCode::IM_A_TEAPOT, "I'm a teapot!") // throw
    ///         })
    ///         .run_local()?;
    ///     spawn(server);
    ///     let resp = reqwest::get(&format!("http://{}", addr)).await?;
    ///     assert_eq!(StatusCode::IM_A_TEAPOT, resp.status());
    ///     assert_eq!("I'm a teapot!", resp.text().await?);
    ///     Ok(())
    /// }
    /// ```
    pub status_code: StatusCode,

    /// Error kind, is inferred automatically by status code.
    pub kind: ErrorKind,

    /// Data will be written to response body if self.expose is true.
    /// StatusCode will be responded to client if Error is thrown by the top middleware.
    ///
    /// ### Example
    /// ```rust
    /// use roa_core::{App, throw, Error};
    /// use async_std::task::spawn;
    /// use http::{StatusCode};
    ///
    /// #[tokio::test]
    /// async fn exposed() -> Result<(), Box<dyn std::error::Error>> {
    ///     let (addr, server) = App::new(())
    ///         .gate(|_ctx, _next| async {
    ///             throw(StatusCode::IM_A_TEAPOT, "I'm a teapot!") // throw
    ///         })
    ///         .run_local()?;
    ///     spawn(server);
    ///     let resp = reqwest::get(&format!("http://{}", addr)).await?;
    ///     assert_eq!(StatusCode::IM_A_TEAPOT, resp.status());
    ///     assert_eq!("I'm a teapot!", resp.text().await?);
    ///     Ok(())
    /// }
    ///
    /// #[tokio::test]
    /// async fn not_exposed() -> Result<(), Box<dyn std::error::Error>> {
    ///     let (addr, server) = App::new(())
    ///         .gate(|_ctx, _next| async {
    ///             Err(Error::new(StatusCode::IM_A_TEAPOT, "I'm a teapot!", false)) // throw
    ///         })
    ///         .run_local()?;
    ///     spawn(server);
    ///     let resp = reqwest::get(&format!("http://{}", addr)).await?;
    ///     assert_eq!(StatusCode::IM_A_TEAPOT, resp.status());
    ///     assert_eq!("", resp.text().await?);
    ///     Ok(())
    /// }
    /// ```
    pub message: String,

    /// if message exposed.
    pub expose: bool,
}

/// Kind of Error.
#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub enum ErrorKind {
    /// [[RFC7231, Section 6.2](https://tools.ietf.org/html/rfc7231#section-6.2)]
    Informational,

    /// [[RFC7231, Section 6.4](https://tools.ietf.org/html/rfc7231#section-6.4)]
    Redirection,

    /// [[RFC7231, Section 6.5](https://tools.ietf.org/html/rfc7231#section-6.5)]
    ClientError,

    /// [[RFC7231, Section 6.6](https://tools.ietf.org/html/rfc7231#section-6.6)]
    ServerError,
}

impl ErrorKind {
    fn infer(status_code: StatusCode) -> Self {
        use ErrorKind::*;
        match status_code.as_u16() / 100 {
            1 => Informational,
            3 => Redirection,
            4 => ClientError,
            5 => ServerError,
            _ => panic!(
                r"status {} cannot be thrown.
                  Please use `ctx.resp_mut().await.status = xxx` to set it.
               ",
                status_code
            ),
        }
    }
}

impl Error {
    /// Construct an error.
    pub fn new(status_code: StatusCode, message: impl ToString, expose: bool) -> Self {
        Self {
            status_code,
            kind: ErrorKind::infer(status_code),
            message: message.to_string(),
            expose,
        }
    }

    pub(crate) fn need_throw(&self) -> bool {
        self.kind == ErrorKind::ServerError
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
