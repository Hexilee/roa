use crate::Result;
use std::future::Future;
use std::pin::Pin;

/// Type of the second parameter in a middleware.
///
/// `Next` is usually a closure capturing the next middleware, context and the next `Next`.
///
/// Developer of middleware can jump to next middleware by calling `next.await`.
///
/// ### Example
///
/// ```rust
/// use roa_core::App;
/// use async_std::task::spawn;
/// use http::StatusCode;
///
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let (addr, server) = App::new(())
///         .gate_fn(|mut ctx, next| async move {
///             ctx.store("id", "1".to_string());
///             next.await?;
///             assert_eq!("5", &*ctx.load::<String>("id").unwrap());
///             Ok(())
///         })
///         .gate_fn(|mut ctx, next| async move {
///             assert_eq!("1", &*ctx.load::<String>("id").unwrap());
///             ctx.store("id", "2".to_string());
///             next.await?;
///             assert_eq!("4", &*ctx.load::<String>("id").unwrap());
///             ctx.store("id", "5".to_string());
///             Ok(())
///         })
///         .gate_fn(|mut ctx, next| async move {
///             assert_eq!("2", &*ctx.load::<String>("id").unwrap());
///             ctx.store("id", "3".to_string());
///             next.await?; // next is none; do nothing
///             assert_eq!("3", &*ctx.load::<String>("id").unwrap());
///             ctx.store("id", "4".to_string());
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
///             if let Err(err) = next.await {
///                 // teapot is ok
///                 if err.status_code != StatusCode::IM_A_TEAPOT {
///                     return Err(err);
///                 }
///             }
///             Ok(())
///         })
///         .gate_fn(|ctx, next| async move {
///             next.await?; // just throw
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
///
pub type Next<R = ()> = Pin<Box<dyn 'static + Future<Output = Result<R>>>>;

pub fn last() -> Next {
    Box::pin(async move { Ok(()) })
}
