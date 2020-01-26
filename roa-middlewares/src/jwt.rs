use crate::{Middleware, Next, State};
use roa::{Context, Status};
use std::future::Future;
use std::pin::Pin;

pub struct Jwt {}

impl<S: State> Middleware<S> for Jwt {
    existential type StatusFuture: 'static + Future<Output = Result<(), Status>> + Send;
    fn handle(&self, ctx: Context<S>, next: Next) -> Self::StatusFuture {
        unimplemented!()
    }
}
