use crate::{Context, State, StatusFuture};

pub type Next<S> = Box<dyn FnOnce(Context<S>) -> StatusFuture + Sync + Send>;
pub fn _next<S: State>(_ctx: Context<S>) -> StatusFuture {
    Box::pin(async move { Ok(()) })
}
