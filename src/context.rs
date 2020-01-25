use crate::{Model, Request, Response, Service};

pub struct Context<M: Model = ()> {
    pub request: Request,
    pub response: Response,
    pub app: Service<M>,
    pub model: M,
}

impl<M: Model> Context<M> {
    pub fn new(request: Request, app: Service<M>) -> Self {
        let model = Model::init(&app);
        Self {
            request,
            response: Response::new(),
            app,
            model,
        }
    }
}
