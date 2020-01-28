pub trait State: 'static + Send + Sized {
}

pub trait Model: 'static + Send + Sync + Sized {
    type State: State;
    fn new_state(&self) -> Self::State;
}

impl Model for () {
    type State = ();
    fn new_state(&self) -> Self::State {}
}

impl State for () {
}
