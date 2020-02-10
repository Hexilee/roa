use crate::{Context, Error, Model, Next, Result};
use async_std::sync::Arc;
use async_trait::async_trait;
use std::future::Future;

/// The Handler trait. An endpoint is a `Handler<M>`.
///
/// Return type of `async` block/function is opaque,
/// you cannot store it as a trait object because you don't know `Handler::HandleFuture`.
///
/// ### HandleFuture Unknown
/// ```rust,compile_fail
/// use roa_core::{Handler, Context, Result, Model};
///
/// // endpoint
/// async fn get(_ctx: Context<()>) -> Result {
///     Ok(())
/// }
///
/// // `Handler::HandleFuture` is unknown.
/// let get_handler: Box<dyn Handler<(), HandleFuture = ?>> = Box::new(get);
/// ```
///
/// ### Dynamic
///
/// Any `Box<Handler>` can be convert to `Box<DynHandler>` by `Handler::dynamic` method.
///
/// ```rust
/// use roa_core::{Endpoint, Context, Result, Model, ResultFuture};
///
/// // endpoint
/// async fn get(_ctx: Context<()>) -> Result {
///     Ok(())
/// }
///
/// // convert to `DynHandler`
/// let dyn_handler: Box<dyn Endpoint<()>> = Box::new(get);
///
/// ```
#[async_trait]
pub trait Endpoint<M>: 'static + Sync + Send
where
    M: Model,
{
    /// Handle context then return a future to get status.
    async fn handle(self: Arc<Self>, ctx: Context<M>) -> Result;
}

#[async_trait]
impl<M, F, T> Endpoint<M> for T
where
    M: Model,
    F: 'static + Future<Output = Result> + Send,
    T: 'static + Sync + Send + Fn(Context<M>) -> F,
{
    async fn handle(self: Arc<Self>, ctx: Context<M>) -> Result {
        (self)(ctx).await
    }
}

/// The TargetHandler trait. A middleware is a `TargetHandler<M, Next>`.
///
/// Return type of `async` block/function is opaque,
/// you cannot store it as a trait object because you don't know `TargetHandler::HandleFuture`.
///
/// ### HandleFuture Unknown
/// ```rust,compile_fail
/// use roa_core::{TargetHandler, Context, Result, Model, Next};
///
/// // middleware
/// async fn middleware(_ctx: Context<()>, next: Next) -> Result {
///     next().await
/// }
///
/// // `TargetHandler::HandleFuture` is unknown.
/// let middleware: Box<dyn TargetHandler<(), Next, HandleFuture = ?>> = Box::new(middleware);
/// ```
///
/// ### Dynamic
///
/// Any `Box<TargetHandler>` can be convert to `Box<DynTargetHandler>` by `TargetHandler::dynamic` method.
///
/// ```rust
/// use roa_core::{Context, Result, Model, Next, Middleware};
///
/// // middleware
/// async fn middleware(_ctx: Context<()>, next: Next) -> Result {
///     next().await
/// }
///
/// // convert to `DynHandler`
/// let dyn_middleware: Box<dyn Middleware<()>> = Box::new(middleware);
///
/// ```
#[async_trait]
pub trait Middleware<M: Model>: 'static + Sync + Send {
    /// Handle context and target, then return a future to get status.
    async fn handle(self: Arc<Self>, ctx: Context<M>, next: Next) -> Result;
}

#[async_trait]
impl<M, F, T> Middleware<M> for T
where
    M: Model,
    F: 'static + Future<Output = Result> + Send,
    T: 'static + Sync + Send + Fn(Context<M>, Next) -> F,
{
    #[inline]
    async fn handle(self: Arc<Self>, ctx: Context<M>, next: Next) -> Result {
        (self)(ctx, next).await
    }
}

#[async_trait]
pub trait ErrorHandler<M: Model>: 'static + Sync + Send {
    /// Handle context and target, then return a future to get status.
    async fn handle(self: Arc<Self>, ctx: Context<M>, err: Error) -> Result;
}

#[async_trait]
impl<M, F, T> ErrorHandler<M> for T
where
    M: Model,
    F: 'static + Future<Output = Result> + Send,
    T: 'static + Sync + Send + Fn(Context<M>, Error) -> F,
{
    #[inline]
    async fn handle(self: Arc<Self>, ctx: Context<M>, err: Error) -> Result {
        (self)(ctx, err).await
    }
}

pub async fn default_error_handler<M: Model>(context: Context<M>, err: Error) -> Result {
    context.resp_mut().await.status = err.status_code;
    if err.expose {
        context.resp_mut().await.write_str(&err.message);
    }
    if err.need_throw() {
        Err(err)
    } else {
        Ok(())
    }
}
