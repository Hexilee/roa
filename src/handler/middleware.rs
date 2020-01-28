use crate::{DynTargetHandler, Model, Next, TargetHandler};

pub type DynMiddleware<M> = DynTargetHandler<M, Next>;

pub trait Middleware<M: Model>: TargetHandler<M, Next> {}

impl<M, T> Middleware<M> for T
where
    M: Model,
    T: TargetHandler<M, Next>,
{
}
