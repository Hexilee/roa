use crate::{Context, State, Status, StatusFuture};
use std::future::Future;

pub type DynStatusHandler<S> = dyn 'static + Sync + Send + Fn(Context<S>, Status) -> StatusFuture;

pub trait StatusHandler<S: State>: 'static + Sync + Send {
    type StatusFuture: 'static + Future<Output = Result<(), Status>> + Send;
    fn handle(&self, ctx: Context<S>, status: Status) -> Self::StatusFuture;
    fn dynamic(self: Box<Self>) -> Box<DynStatusHandler<S>> {
        Box::new(move |ctx, status| Box::pin(self.handle(ctx, status)))
    }
}

impl<S, F, T> StatusHandler<S> for T
where
    S: State,
    F: 'static + Future<Output = Result<(), Status>> + Send,
    T: 'static + Sync + Send + Fn(Context<S>, Status) -> F,
{
    type StatusFuture = F;
    fn handle(&self, ctx: Context<S>, status: Status) -> Self::StatusFuture {
        (self)(ctx, status)
    }
}

pub async fn default_status_handler<S: State>(
    mut context: Context<S>,
    status: Status,
) -> Result<(), Status> {
    context.response.status(status.status_code);
    if status.expose {
        context.response.write_str(&status.message);
    }
    if status.need_throw() {
        Err(status)
    } else {
        Ok(())
    }
}
