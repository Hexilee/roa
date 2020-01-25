use crate::Service;

pub trait Model: 'static + Sync + Send + Sized {
    fn init(app: &Service<Self>) -> Self;
}

impl Model for () {
    fn init(_app: &Service<Self>) -> Self {
        ()
    }
}
