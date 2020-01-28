pub trait State: 'static + Send + Sized {
    type Model: Model<State = Self>;
}

pub trait Model: 'static + Send + Sync + Sized {
    type State: State<Model = Self>;
    fn new_state(&self) -> Self::State;
}

impl Model for () {
    type State = ();
    fn new_state(&self) -> Self::State {}
}

impl State for () {
    type Model = ();
}
