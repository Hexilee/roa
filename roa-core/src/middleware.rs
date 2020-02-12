use crate::{last, Context, Next, Result, State};
use async_std::sync::Arc;
use async_trait::async_trait;
use std::future::Future;

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
/// use roa_core::{Context, Result, Next, Middleware};
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
pub trait Middleware<S: State>: 'static + Sync + Send {
    /// Handle context and target, then return a future to get status.
    async fn handle(self: Arc<Self>, ctx: Context<S>, next: Next) -> Result;
    async fn end(self: Arc<Self>, ctx: Context<S>) -> Result {
        self.handle(ctx, Box::new(last)).await
    }
}

#[async_trait]
impl<S, F, T> Middleware<S> for T
where
    S: State,
    F: 'static + Future<Output = Result> + Send,
    T: 'static + Sync + Send + Fn(Context<S>, Next) -> F,
{
    #[inline]
    async fn handle(self: Arc<Self>, ctx: Context<S>, next: Next) -> Result {
        (self)(ctx, next).await
    }
}

#[async_trait]
impl<S, F> Middleware<S> for fn(Context<S>) -> F
where
    S: State,
    F: 'static + Future<Output = Result> + Send,
{
    #[inline]
    async fn handle(self: Arc<Self>, ctx: Context<S>, _next: Next) -> Result {
        (self)(ctx).await
    }
}
