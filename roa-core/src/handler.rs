use crate::{Context, Error, Model, Result, ResultFuture};
use std::future::Future;

pub type DynHandler<M, R = ()> = dyn 'static + Sync + Send + Fn(Context<M>) -> ResultFuture<R>;
pub type DynTargetHandler<M, Target, R = ()> =
    dyn 'static + Sync + Send + Fn(Context<M>, Target) -> ResultFuture<R>;

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
