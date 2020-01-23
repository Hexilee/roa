use crate::{App, Model};
use hyper::{Body, Request};

pub struct Context<M: Model = ()> {
    request: Request<Body>,
    app: &'static App<M>,
    model: M,
}

impl<M: Model> Context<M> {
    pub fn new(request: Request<Body>, app: &'static App<M>) -> Self {
        Self {
            request,
            app,
            model: Model::init(app),
        }
    }
}
