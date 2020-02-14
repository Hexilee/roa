use crate::ResultFuture;

/// Type of the second parameter in a middleware.
///
/// `Next` is usually a closure capturing the next middleware, context and the next `Next`.
///
/// Developer of middleware can jump to next middleware by calling `next().await`.
///
/// ### Example
///
/// ```rust
/// use roa_core::App;
/// use async_std::task::spawn;
/// use http::StatusCode;
///
/// struct Symbol;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let (addr, server) = App::new(())
///         .gate_fn(|mut ctx, next| async move {
///             ctx.store::<Symbol>("id", "1".to_string()).await;
///             next().await?;
///             assert_eq!("5", ctx.load::<Symbol>("id").await.unwrap().as_ref());
///             Ok(())
///         })
///         .gate_fn(|mut ctx, next| async move {
///             assert_eq!("1", ctx.load::<Symbol>("id").await.unwrap().as_ref());
///             ctx.store::<Symbol>("id", "2".to_string()).await;
///             next().await?;
///             assert_eq!("4", ctx.load::<Symbol>("id").await.unwrap().as_ref());
///             ctx.store::<Symbol>("id", "5".to_string()).await;
///             Ok(())
///         })
///         .gate_fn(|mut ctx, next| async move {
///             assert_eq!("2", ctx.load::<Symbol>("id").await.unwrap().as_ref());
///             ctx.store::<Symbol>("id", "3".to_string()).await;
///             next().await?; // next is none; do nothing
///             assert_eq!("3", ctx.load::<Symbol>("id").await.unwrap().as_ref());
///             ctx.store::<Symbol>("id", "4".to_string()).await;
///             Ok(())
///         })
///         .run_local()?;
///     spawn(server);
///     let resp = reqwest::get(&format!("http://{}", addr)).await?;
///     assert_eq!(StatusCode::OK, resp.status());
///     Ok(())
/// }
/// ```
///
/// ### Error Handling
///
/// You can catch or straightly throw a Error returned by next.
///
/// ```rust
/// use roa_core::{App, throw};
/// use async_std::task::spawn;
/// use http::StatusCode;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let (addr, server) = App::new(())
///         .gate_fn(|ctx, next| async move {
///             // catch
///             if let Err(err) = next().await {
///                 // teapot is ok
///                 if err.status_code != StatusCode::IM_A_TEAPOT {
///                     return Err(err);
///                 }
///             }
///             Ok(())
///         })
///         .gate_fn(|ctx, next| async move {
///             next().await?; // just throw
///             unreachable!()
///         })
///         .gate_fn(|_ctx, _next| async move {
///             throw!(StatusCode::IM_A_TEAPOT, "I'm a teapot!")
///         })
///         .run_local()?;
///     spawn(server);
///     let resp = reqwest::get(&format!("http://{}", addr)).await?;
///     assert_eq!(StatusCode::OK, resp.status());
///     Ok(())
/// }
/// ```
pub type Next = Box<dyn FnOnce() -> ResultFuture + Sync + Send>;

pub fn last() -> ResultFuture {
    Box::pin(async move { Ok(()) })
}
