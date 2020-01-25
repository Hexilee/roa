use crate::{Context, State, Status, StatusFuture};

pub trait StatusHandler<S: State>: 'static + Send + Sync {
    fn handle(&self, ctx: Context<S>, status: Status) -> StatusFuture;
}

impl<T, S> StatusHandler<S> for T
where
    S: State,
    T: 'static + Send + Sync + Fn(Context<S>, Status) -> StatusFuture,
{
    fn handle(&self, ctx: Context<S>, status: Status) -> StatusFuture {
        self(ctx, status)
    }
}

pub fn make_status_handler<S: State>(
    handler: impl 'static + Sync + Send + Fn(Context<S>, Status) -> StatusFuture,
) -> Box<dyn StatusHandler<S>> {
    Box::new(handler)
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
