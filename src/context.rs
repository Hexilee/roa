use crate::{App, Model, Request, Response};

pub struct Context<M: Model = ()> {
    pub request: Request,
    pub response: Response,
    pub app: &'static App<M>,
    pub model: M,
}

impl<M: Model> Context<M> {
    pub fn new(request: Request, app: &'static App<M>) -> Self {
        Self {
            request,
            response: Response::new(),
            app,
            model: Model::init(app),
        }
    }
}
