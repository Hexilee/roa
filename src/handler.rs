mod middleware;
mod status_handler;
use crate::{Context, State, Status, StatusFuture};
pub use middleware::{first_middleware, DynMiddleware, Middleware};
pub use status_handler::{default_status_handler, DynStatusHandler, StatusHandler};
use std::future::Future;

pub type DynHandler<S> = dyn 'static + Sync + Send + Fn(Context<S>) -> StatusFuture;
pub type DynTargetHandler<S, Target> =
    dyn 'static + Sync + Send + Fn(Context<S>, Target) -> StatusFuture;

pub trait Handler<S: State>: 'static + Sync + Send {
    type StatusFuture: 'static + Future<Output = Result<(), Status>> + Send;
    fn handle(&self, ctx: Context<S>) -> Self::StatusFuture;
    fn dynamic(self: Box<Self>) -> Box<DynHandler<S>> {
        Box::new(move |ctx| Box::pin(self.handle(ctx)))
    }
}

impl<S, F, T> Handler<S> for T
where
    S: State,
    F: 'static + Future<Output = Result<(), Status>> + Send,
    T: 'static + Sync + Send + Fn(Context<S>) -> F,
{
    type StatusFuture = F;
    fn handle(&self, ctx: Context<S>) -> Self::StatusFuture {
        (self)(ctx)
    }
}

pub trait TargetHandler<S: State, Target>: 'static + Sync + Send {
    type StatusFuture: 'static + Future<Output = Result<(), Status>> + Send;
    fn handle(&self, ctx: Context<S>, target: Target) -> Self::StatusFuture;
    fn dynamic(self: Box<Self>) -> Box<DynTargetHandler<S, Target>> {
        Box::new(move |ctx, target| Box::pin(self.handle(ctx, target)))
    }
}

impl<S, F, Target, T> TargetHandler<S, Target> for T
where
    S: State,
    F: 'static + Future<Output = Result<(), Status>> + Send,
    T: 'static + Sync + Send + Fn(Context<S>, Target) -> F,
{
    type StatusFuture = F;
    fn handle(&self, ctx: Context<S>, target: Target) -> Self::StatusFuture {
        (self)(ctx, target)
    }
}
