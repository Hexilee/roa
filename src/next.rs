use crate::{Context, State, StatusFuture};

pub type Next<S> = Box<dyn FnOnce(&mut Context<S>) -> StatusFuture + Sync + Send>;
pub fn _next<S: State>(_ctx: &mut Context<S>) -> StatusFuture {
    Box::pin(async { Ok(()) })
}
