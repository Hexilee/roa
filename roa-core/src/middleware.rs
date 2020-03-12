use crate::{async_trait, Context, Result, State};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

/// ### Middleware
///
/// There are two kinds of middlewares,
/// the one is functional middlewares, the another is trait middlewares.
///
/// #### Functional Middlewares
///
/// A normal functional middleware is an object implements `Fn` trait:
///
/// ```rust
/// use roa_core::{Context, Next, Result, Middleware};
/// use std::future::Future;
///
/// fn is_middleware<S>(middleware: impl for<'a> Middleware<'a, S>) {
/// }
///
/// async fn middleware(ctx: &mut Context<()>, next: Next<'_>) -> Result {
///     Ok(())
/// }
///
/// is_middleware(middleware);
/// ```
///
/// Closures are also supported, but feature(async_closure) is required:
///
/// #### Trait Middlewares
///
/// A trait middleware is an object implementing trait `Middleware`.
///
/// ```rust
/// use roa_core::{Middleware, Context, Next, Result, async_trait};
/// use async_std::sync::Arc;
/// use std::time::Instant;
///
/// fn is_middleware<S>(middleware: impl for<'a> Middleware<'a, S>) {}
///
/// struct Logger;
///
/// #[async_trait(?Send)]
/// impl <'a> Middleware<'a, ()> for Logger {
///     async fn handle(&'a self, ctx: &'a mut Context<()>, next: Next<'a>) -> Result {
///         let start = Instant::now();
///         let result = next.await;
///         println!("time elapsed: {}ms", start.elapsed().as_millis());
///         result
///     }
/// }
///
/// is_middleware(Logger);
/// ```
#[async_trait(?Send)]
pub trait Middleware<'a, S>: 'static + Sync + Send {
    /// Handle context and next, then return a future to get status.
    async fn handle(&'a self, ctx: &'a mut Context<S>, next: Next<'a>) -> Result;
}

#[async_trait(?Send)]
impl<'a, S, T, F> Middleware<'a, S> for T
where
    S: 'a,
    T: 'static + Send + Sync + Fn(&'a mut Context<S>, Next<'a>) -> F,
    F: 'a + Future<Output = Result>,
{
    #[inline]
    async fn handle(&'a self, ctx: &'a mut Context<S>, next: Next<'a>) -> Result {
        (self)(ctx, next).await
    }
}

/// ### Endpoint
///
/// There are two kinds of endpoints,
/// the one is functional endpoints, the another is trait endpoints.
///
/// #### Functional Endpoints
///
/// A normal functional endpoint is an object implements `Fn` trait:
///
/// ```rust
/// use roa_core::{Context, Next, Result, Endpoint};
/// use std::future::Future;
///
/// fn is_endpoint<S>(endpoint: impl for<'a> Endpoint<'a, S>) {
/// }
///
/// async fn endpoint(ctx: &mut Context<()>) -> Result {
///     Ok(())
/// }
///
/// is_endpoint(endpoint);
/// ```
///
/// Closures are also supported, but feature(async_closure) is required:
///
/// #### Trait Endpoints
///
/// A trait endpoint is an object implementing trait `Endpoint`.
///
/// ```rust
/// use roa_core::{Endpoint, Context, Next, Result, async_trait};
/// use async_std::sync::Arc;
/// use std::time::Instant;
///
/// fn is_endpoint<S>(endpoint: impl for<'a> Endpoint<'a, S>) {
/// }
///
/// struct Logger;
///
/// #[async_trait(?Send)]
/// impl <'a> Endpoint<'a, ()> for Logger {
///     async fn end(&'a self, ctx: &'a mut Context<()>) -> Result {
///         Ok(())
///     }
/// }
///
/// is_endpoint(Logger);
/// ```
#[async_trait(?Send)]
pub trait Endpoint<'a, S>: 'static + Sync + Send {
    #[inline]
    async fn end(&'a self, ctx: &'a mut Context<S>) -> Result;
}

#[async_trait(?Send)]
impl<'a, S, T, F> Endpoint<'a, S> for T
where
    S: 'a,
    T: 'static + Send + Sync + Fn(&'a mut Context<S>) -> F,
    F: 'a + Future<Output = Result>,
{
    #[inline]
    async fn end(&'a self, ctx: &'a mut Context<S>) -> Result {
        (self)(ctx).await
    }
}

/// Type of the second parameter in a middleware.
///
/// `Next` is usually a closure capturing the next middleware, context and the next `Next`.
///
/// Developer of middleware can jump to next middleware by calling `next.await`.
///
/// ### Example
///
/// ```rust
/// use roa_core::{App, Context, Result, Error, MiddlewareExt, Next};
/// use roa_core::http::StatusCode;
///
/// let mut app = App::new((), first.chain(second).chain(third).end(end));
/// async fn first(ctx: &mut Context<()>, next: Next<'_>) -> Result {
///     ctx.store("id", "1".to_string());
///     next.await?;
///     assert_eq!("5", &*ctx.load::<String>("id").unwrap());
///     Ok(())
/// }
/// async fn second(ctx: &mut Context<()>, next: Next<'_>) -> Result {
///     assert_eq!("1", &*ctx.load::<String>("id").unwrap());
///     ctx.store("id", "2".to_string());
///     next.await?;
///     assert_eq!("4", &*ctx.load::<String>("id").unwrap());
///     ctx.store("id", "5".to_string());
///     Ok(())
/// }
/// async fn third(ctx: &mut Context<()>, next: Next<'_>) -> Result {
///     assert_eq!("2", &*ctx.load::<String>("id").unwrap());
///     ctx.store("id", "3".to_string());
///     next.await?; // next is none; do nothing
///     assert_eq!("3", &*ctx.load::<String>("id").unwrap());
///     ctx.store("id", "4".to_string());
///     Ok(())
/// }
///
/// async fn end(ctx: &mut Context<()>) -> Result {
///     assert_eq!("3", &*ctx.load::<String>("id").unwrap());
///     Ok(())
/// }
/// ```
///
/// ### Error Handling
///
/// You can catch or straightly throw a Error returned by next.
///
/// ```rust
/// use roa_core::{App, Context, Result, Error, MiddlewareExt, Next, throw};
/// use roa_core::http::StatusCode;
///         
/// let mut app = App::new((), catch.chain(gate).end(end));
///
/// async fn catch(ctx: &mut Context<()>, next: Next<'_>) -> Result {
///     // catch
///     if let Err(err) = next.await {
///         // teapot is ok
///         if err.status_code != StatusCode::IM_A_TEAPOT {
///             return Err(err)
///         }
///     }
///     Ok(())
/// }
/// async fn gate(ctx: &mut Context<()>, next: Next<'_>) -> Result {
///     next.await?; // just throw
///     unreachable!()
/// }
///
/// async fn end(ctx: &mut Context<()>) -> Result {
///     throw!(StatusCode::IM_A_TEAPOT, "I'm a teapot!")
/// }
/// ```
///
pub type Next<'a> = &'a mut (dyn Unpin + Future<Output = Result<()>>);
