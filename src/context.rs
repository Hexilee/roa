use crate::{Model, StaticApp};
use hyper::{Body, Request};

pub struct Context<'a, M: Model = ()> {
    request: Request<Body>,
    app: &'a StaticApp<M>,
    model: M,
}

impl<'a, M: Model> Context<'a, M> {
    pub fn new(request: Request<Body>, app: &'a StaticApp<M>) -> Self {
        Self {
            request,
            app,
            model: Model::init(app),
        }
    }
}
