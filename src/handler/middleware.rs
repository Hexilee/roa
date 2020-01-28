use crate::{DynTargetHandler, Next, State, TargetHandler};

pub type DynMiddleware<S> = DynTargetHandler<S, Next>;

pub trait Middleware<S: State>: TargetHandler<S, Next> {}

impl<S, T> Middleware<S> for T
where
    S: State,
    T: TargetHandler<S, Next>,
{
}
