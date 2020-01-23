use crate::Application;

pub trait Model: Sync + Send + Sized {
    fn init(app: &dyn Application) -> Self;
}

impl Model for () {
    fn init(_app: &dyn Application) -> Self {
        ()
    }
}
