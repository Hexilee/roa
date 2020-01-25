use crate::{Request, Response, Service};

pub struct Context<S: State> {
    pub request: Request,
    pub response: Response,
    pub app: Service<S>,
    pub state: S,
}

impl<S: State> Context<S> {
    pub fn new(request: Request, app: Service<S>) -> Self {
        Self {
            request,
            response: Response::new(),
            app,
            state: Default::default(),
        }
    }
}

pub trait State: 'static + Send + Default {}
impl<T> State for T where T: 'static + Send + Default {}
