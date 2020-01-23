use crate::{Application, Model};
use hyper::{Body, Request};

pub struct Context<'a, M: Model = ()> {
    request: Request<Body>,
    app: &'a (dyn Application + Sync + Send),
    model: M,
}

impl<'a, M: Model> Context<'a, M> {
    pub fn new(request: Request<Body>, app: &'a (dyn Application + Sync + Send)) -> Self {
        Self {
            request,
            app,
            model: Model::init(app),
        }
    }
}
