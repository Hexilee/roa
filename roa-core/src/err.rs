pub use http::StatusCode;
use std::fmt::{Display, Formatter};
use std::future::Future;
use std::pin::Pin;
use std::result::Result as StdResult;

/// Type alias for `Pin<Box<dyn 'a + Future<Output = Result<R>>>>`.
pub type ResultFuture<'a, R = ()> = Pin<Box<dyn 'a + Future<Output = Result<R>>>>;

/// Type alias for `StdResult<R, Error>`.
pub type Result<R = ()> = StdResult<R, Status>;

/// Throw an `Err(Status)`.
///
/// - `throw!(status_code)` will be expanded to `throw!(status_code, "")`
/// - `throw!(status_code, message)` will be expanded to `throw!(status_code, message, true)`
/// - `throw!(status_code, message, expose)` will be expanded to `return Err(Error::new(status_code, message, expose));`
///
/// ### Example
/// ```rust
/// use roa_core::{App, Context, Next, Result, throw};
/// use roa_core::http::StatusCode;
///
/// let app = App::new(()).gate(gate).end(end);
/// async fn gate(ctx: &mut Context<()>, next: Next<'_>) -> Result {
///     next.await?; // throw
///     unreachable!();
///     ctx.resp.status = StatusCode::OK;
///     Ok(())
/// }
///
/// async fn end(ctx: &mut Context<()>) -> Result {
///     throw!(StatusCode::IM_A_TEAPOT, "I'm a teapot!"); // throw
///     unreachable!()
/// }
/// ```
#[macro_export]
macro_rules! throw {
    ($status_code:expr) => {
        $crate::throw!($status_code, "");
    };
    ($status_code:expr, $message:expr) => {
        $crate::throw!($status_code, $message, true);
    };
    ($status_code:expr, $message:expr, $expose:expr) => {
        return Err($crate::Status::new($status_code, $message, $expose));
    };
}

/// The `Status` of roa.
#[derive(Debug, Clone)]
pub struct Status {
    /// StatusCode will be responded to client if Error is thrown by the top middleware.
    ///
    /// ### Example
    /// ```rust
    /// use roa_core::{App, Context, Next, Result, MiddlewareExt, throw};
    /// use roa_core::http::StatusCode;
    ///
    /// let app = App::new(()).gate(gate).end(end);
    /// async fn gate(ctx: &mut Context<()>, next: Next<'_>) -> Result {
    ///     ctx.resp.status = StatusCode::OK;
    ///     next.await // not caught
    /// }
    ///
    /// async fn end(ctx: &mut Context<()>) -> Result {
    ///     throw!(StatusCode::IM_A_TEAPOT, "I'm a teapot!"); // throw
    ///     unreachable!()
    /// }
    /// ```
    pub status_code: StatusCode,

    /// Data will be written to response body if self.expose is true.
    /// StatusCode will be responded to client if Error is thrown by the top middleware.
    ///
    /// ### Example
    /// ```rust
    /// use roa_core::{App, Context, Result, Status};
    /// use roa_core::http::StatusCode;
    ///
    /// let app = App::new(()).end(end);
    ///
    /// async fn end(ctx: &mut Context<()>) -> Result {
    ///     Err(Status::new(StatusCode::IM_A_TEAPOT, "I'm a teapot!", false)) // message won't be exposed to user.
    /// }
    ///
    /// ```
    pub message: String,

    /// if message exposed.
    pub expose: bool,
}

/// A error wrapper for status.
#[derive(Debug)]
pub struct StatusError(Status);

impl Status {
    /// Construct an error.
    #[inline]
    pub fn new(status_code: StatusCode, message: impl ToString, expose: bool) -> Self {
        Self {
            status_code,
            message: message.to_string(),
            expose,
        }
    }

    #[inline]
    pub(crate) fn need_throw(&self) -> bool {
        self.status_code.as_u16() / 100 == 5
    }
}

impl<E> From<E> for Status
where
    E: std::error::Error,
{
    #[inline]
    fn from(err: E) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, err, false)
    }
}

impl From<Status> for StatusError {
    #[inline]
    fn from(status: Status) -> Self {
        StatusError(status)
    }
}

impl Display for Status {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> StdResult<(), std::fmt::Error> {
        f.write_str(&format!("{}: {}", self.status_code, self.message))
    }
}

impl Display for StatusError {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> StdResult<(), std::fmt::Error> {
        self.0.fmt(f)
    }
}

impl std::error::Error for StatusError {}
