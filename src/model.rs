use crate::App;

pub trait Model: 'static + Sync + Send + Sized {
    fn init(app: &App<Self>) -> Self;
}

impl Model for () {
    fn init(_app: &App<Self>) -> Self {
        ()
    }
}
