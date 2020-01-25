use crate::{Request, Response, Service, Status};
use http::StatusCode;

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

    pub fn throw(&mut self, status_code: StatusCode, message: impl ToString) -> Result<(), Status> {
        Err(Status::new(status_code, message.to_string(), true))
    }
}

pub trait State: 'static + Send + Default {}
impl<T> State for T where T: 'static + Send + Default {}
