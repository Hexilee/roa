use crate::{Context, Model, StatusFuture};

pub type Next<M> = Box<dyn FnOnce(&mut Context<M>) -> StatusFuture + Sync + Send>;
pub fn _next<M: Model>(_ctx: &mut Context<M>) -> StatusFuture {
    Box::pin(async { Ok(()) })
}
