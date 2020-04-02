/// The `State` trait, should be replace with trait alias.
/// The `App::state` will be cloned when a request inbounds.
///
/// `State` is designed to share data or handler between middlewares.
///
/// ### Example
/// ```rust
/// use roa_core::{App, Context, Next, Result};
/// use roa_core::http::StatusCode;
///
/// #[derive(Clone)]
/// struct State {
///     id: u64,
/// }
///
/// let app = App::state(State { id: 0 }).gate(gate).end(end);
/// async fn gate(ctx: &mut Context<State>, next: Next<'_>) -> Result {
///     ctx.id = 1;
///     next.await
/// }
///
/// async fn end(ctx: &mut Context<State>) -> Result {
///     let id = ctx.id;
///     assert_eq!(1, id);
///     Ok(())
/// }
/// ```
pub trait State: 'static + Clone + Send + Sync + Sized {}

impl<T: 'static + Clone + Send + Sync + Sized> State for T {}
