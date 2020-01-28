mod middleware;
mod status_handler;
use crate::{Context, State, Status, StatusFuture};
pub use middleware::{DynMiddleware, Middleware};
pub use status_handler::{default_status_handler, DynStatusHandler, StatusHandler};
use std::future::Future;

pub type DynHandler<S, R = ()> = dyn 'static + Sync + Send + Fn(Context<S>) -> StatusFuture<R>;
pub type DynTargetHandler<S, Target, R = ()> =
    dyn 'static + Sync + Send + Fn(Context<S>, Target) -> StatusFuture<R>;

pub trait Handler<S: State, R = ()>: 'static + Sync + Send {
    type StatusFuture: 'static + Future<Output = Result<R, Status>> + Send;
    fn handle(&self, ctx: Context<S>) -> Self::StatusFuture;
    fn dynamic(self: Box<Self>) -> Box<DynHandler<S, R>> {
        Box::new(move |ctx| Box::pin(self.handle(ctx)))
    }
}

impl<S, R, F, T> Handler<S, R> for T
where
    S: State,
    F: 'static + Future<Output = Result<R, Status>> + Send,
    T: 'static + Sync + Send + Fn(Context<S>) -> F,
{
    type StatusFuture = F;
    fn handle(&self, ctx: Context<S>) -> Self::StatusFuture {
        (self)(ctx)
    }
}

pub trait TargetHandler<S: State, Target, R = ()>: 'static + Sync + Send {
    type StatusFuture: 'static + Future<Output = Result<R, Status>> + Send;
    fn handle(&self, ctx: Context<S>, target: Target) -> Self::StatusFuture;
    fn dynamic(self: Box<Self>) -> Box<DynTargetHandler<S, Target, R>> {
        Box::new(move |ctx, target| Box::pin(self.handle(ctx, target)))
    }
}

impl<S, F, Target, R, T> TargetHandler<S, Target, R> for T
where
    S: State,
    F: 'static + Future<Output = Result<R, Status>> + Send,
    T: 'static + Sync + Send + Fn(Context<S>, Target) -> F,
{
    type StatusFuture = F;
    fn handle(&self, ctx: Context<S>, target: Target) -> Self::StatusFuture {
        (self)(ctx, target)
    }
}
