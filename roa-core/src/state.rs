/// The `State` trait, should be replace with trait alias.
/// The `App::state` will be cloned when a request inbounds.
///
/// `State` is designed to share data or handler between middlewares.
///
/// ### Example
/// ```rust
/// use roa_core::App;
/// use roa_core::http::StatusCode;
///
/// #[derive(Clone)]
/// struct State {
///     id: u64,
/// }
///
/// let mut app = App::new(State { id: 0 });
/// app.gate_fn(|mut ctx, next| async move {
///     ctx.id = 1;
///     next.await
/// });
/// app.end(|ctx| async move {
///     let id = ctx.id;
///     assert_eq!(1, id);
///     Ok(())
/// });
/// ```
pub trait State: 'static + Clone + Send + Sync + Sized {}

impl<T: 'static + Clone + Send + Sync + Sized> State for T {}
