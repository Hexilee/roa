use crate::{Context, DynTargetHandler, Next, State, Status, TargetHandler};

pub type DynMiddleware<S> = DynTargetHandler<S, Next>;

pub trait Middleware<S: State>: TargetHandler<S, Next> {}

impl<S, T> Middleware<S> for T
where
    S: State,
    T: TargetHandler<S, Next>,
{
}

pub async fn first_middleware<S: State>(_ctx: Context<S>, next: Next) -> Result<(), Status> {
    next().await
}
