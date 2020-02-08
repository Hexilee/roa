use crate::{Context, Error, Model, Result, ResultFuture};
use std::future::Future;

/// A Handler whose return type is ResultFuture.
pub type DynHandler<M, R = ()> = dyn 'static + Sync + Send + Fn(Context<M>) -> ResultFuture<R>;

/// A TargetHandler whose return type is ResultFuture.
pub type DynTargetHandler<M, Target, R = ()> =
    dyn 'static + Sync + Send + Fn(Context<M>, Target) -> ResultFuture<R>;

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
/// use roa_core::{Handler, Context, Result, Model, DynHandler, ResultFuture};
///
/// // endpoint
/// async fn get(_ctx: Context<()>) -> Result {
///     Ok(())
/// }
///
/// // convert to `DynHandler`
/// let dyn_handler: Box<DynHandler<()>> = Box::new(get).dynamic();
///
/// ```
pub trait Handler<M: Model, R = ()>: 'static + Sync + Send {
    type HandleFuture: 'static + Future<Output = Result<R>> + Send;
    fn handle(&self, ctx: Context<M>) -> Self::HandleFuture;
    fn dynamic(self: Box<Self>) -> Box<DynHandler<M, R>> {
        Box::new(move |ctx| Box::pin(self.handle(ctx)))
    }
}

impl<M, R, F, T> Handler<M, R> for T
where
    M: Model,
    F: 'static + Future<Output = Result<R>> + Send,
    T: 'static + Sync + Send + Fn(Context<M>) -> F,
{
    type HandleFuture = F;
    #[inline]
    fn handle(&self, ctx: Context<M>) -> Self::HandleFuture {
        (self)(ctx)
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
/// use roa_core::{TargetHandler, Context, Result, Model, Next, DynTargetHandler};
///
/// // middleware
/// async fn middleware(_ctx: Context<()>, next: Next) -> Result {
///     next().await
/// }
///
/// // convert to `DynHandler`
/// let dyn_middleware: Box<DynTargetHandler<(), Next>> = Box::new(middleware).dynamic();
///
/// ```
pub trait TargetHandler<M: Model, Target, R = ()>: 'static + Sync + Send {
    type HandleFuture: 'static + Future<Output = Result<R>> + Send;
    fn handle(&self, ctx: Context<M>, target: Target) -> Self::HandleFuture;
    fn dynamic(self: Box<Self>) -> Box<DynTargetHandler<M, Target, R>> {
        Box::new(move |ctx, target| Box::pin(self.handle(ctx, target)))
    }
}

impl<M, F, Target, R, T> TargetHandler<M, Target, R> for T
where
    M: Model,
    F: 'static + Future<Output = Result<R>> + Send,
    T: 'static + Sync + Send + Fn(Context<M>, Target) -> F,
{
    type HandleFuture = F;
    #[inline]
    fn handle(&self, ctx: Context<M>, target: Target) -> Self::HandleFuture {
        (self)(ctx, target)
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
