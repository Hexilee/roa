use crate::StaticApp;

pub trait Model: 'static + Sync + Send + Sized {
    fn init(app: &StaticApp<Self>) -> Self;
}

impl Model for () {
    fn init(_app: &StaticApp<Self>) -> Self {
        ()
    }
}
