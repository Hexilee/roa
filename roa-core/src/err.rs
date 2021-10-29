use std::fmt::{Display, Formatter};
use std::result::Result as StdResult;

pub use http::StatusCode;

/// Type alias for `StdResult`.
pub type Result<R = ()> = StdResult<R, Status>;

/// Construct a `Status`.
///
/// - `status!(status_code)` will be expanded to `status!(status_code, "")`
/// - `status!(status_code, message)` will be expanded to `status!(status_code, message, true)`
/// - `status!(status_code, message, expose)` will be expanded to `Status::new(status_code, message, expose)`
///
/// ### Example
/// ```rust
/// use roa_core::{App, Context, Next, Result, status};
/// use roa_core::http::StatusCode;
///
/// let app = App::new()
///     .gate(gate)
///     .end(status!(StatusCode::IM_A_TEAPOT, "I'm a teapot!"));
/// async fn gate(ctx: &mut Context, next: Next<'_>) -> Result {
///     next.await?; // throw
///     unreachable!();
///     ctx.resp.status = StatusCode::OK;
///     Ok(())
/// }
/// ```
#[macro_export]
macro_rules! status {
    ($status_code:expr) => {
        $crate::status!($status_code, "")
    };
    ($status_code:expr, $message:expr) => {
        $crate::status!($status_code, $message, true)
    };
    ($status_code:expr, $message:expr, $expose:expr) => {
        $crate::Status::new($status_code, $message, $expose)
    };
}

/// Throw an `Err(Status)`.
///
/// - `throw!(status_code)` will be expanded to `throw!(status_code, "")`
/// - `throw!(status_code, message)` will be expanded to `throw!(status_code, message, true)`
/// - `throw!(status_code, message, expose)` will be expanded to `return Err(Status::new(status_code, message, expose));`
///
/// ### Example
/// ```rust
/// use roa_core::{App, Context, Next, Result, throw};
/// use roa_core::http::StatusCode;
///
/// let app = App::new().gate(gate).end(end);
/// async fn gate(ctx: &mut Context, next: Next<'_>) -> Result {
///     next.await?; // throw
///     unreachable!();
///     ctx.resp.status = StatusCode::OK;
///     Ok(())
/// }
///
/// async fn end(ctx: &mut Context) -> Result {
///     throw!(StatusCode::IM_A_TEAPOT, "I'm a teapot!"); // throw
///     unreachable!()
/// }
/// ```
#[macro_export]
macro_rules! throw {
    ($status_code:expr) => {
        return core::result::Result::Err($crate::status!($status_code))
    };
    ($status_code:expr, $message:expr) => {
        return core::result::Result::Err($crate::status!($status_code, $message))
    };
    ($status_code:expr, $message:expr, $expose:expr) => {
        return core::result::Result::Err($crate::status!($status_code, $message, $expose))
    };
}

/// The `Status` of roa.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Status {
    /// StatusCode will be responded to client if Error is thrown by the top middleware.
    ///
    /// ### Example
    /// ```rust
    /// use roa_core::{App, Context, Next, Result, MiddlewareExt, throw};
    /// use roa_core::http::StatusCode;
    ///
    /// let app = App::new().gate(gate).end(end);
    /// async fn gate(ctx: &mut Context, next: Next<'_>) -> Result {
    ///     ctx.resp.status = StatusCode::OK;
    ///     next.await // not caught
    /// }
    ///
    /// async fn end(ctx: &mut Context) -> Result {
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
    /// let app = App::new().end(end);
    ///
    /// async fn end(ctx: &mut Context) -> Result {
    ///     Err(Status::new(StatusCode::IM_A_TEAPOT, "I'm a teapot!", false)) // message won't be exposed to user.
    /// }
    ///
    /// ```
    pub message: String,

    /// if message exposed.
    pub expose: bool,
}

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

impl Display for Status {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> StdResult<(), std::fmt::Error> {
        f.write_str(&format!("{}: {}", self.status_code, self.message))
    }
}
