/// The `State` trait, should be replace with trait alias.
pub trait State: 'static + Send + Sync + Sized {}

/// The `Model` trait.
/// The `new_state` method will be called when a request inbound.
///
/// `Model` and its `State` is designed to share data or handler between middlewares.
/// The only one type implemented `Model` by this crate is `()`, you should implement your custom Model if neccassary.
///
/// ### Example
/// ```rust
/// use roa_core::{App, Model};
/// use log::info;
/// use async_std::task::spawn;
/// use http::StatusCode;
///
/// struct AppModel {
///     default_id: u64,
/// }
///
/// struct AppState {
///     id: u64,
/// }
///
/// impl AppModel {
///     fn new() -> Self {
///         Self {
///             default_id: 0,
///         }
///     }
/// }
///
/// impl Model for AppModel {
///     type State = AppState;
///     fn new_state(&self) -> Self::State {
///         AppState {
///             id: self.default_id,
///         }
///     }
/// }
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let (addr, server) = App::new(AppModel::new())
///         .gate_fn(|ctx, next| async move {
///             ctx.state_mut().await.id = 1;
///             next().await
///         })
///         .end(|ctx| async move {
///             let id = ctx.state().await.id;
///             assert_eq!(1, id);
///             Ok(())
///         })
///         .run_local()?;
///     spawn(server);
///     let resp = reqwest::get(&format!("http://{}", addr)).await?;
///     assert_eq!(StatusCode::OK, resp.status());
///     Ok(())
/// }
/// ```
pub trait Model: 'static + Send + Sync + Sized {
    /// State type of this model.
    type State: State;

    /// Construct a new state instance.
    fn new_state(&self) -> Self::State;
}

impl Model for () {
    type State = ();
    fn new_state(&self) -> Self::State {}
}

impl<T: 'static + Send + Sync + Sized> State for T {}
