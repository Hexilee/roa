/// The `State` trait, should be replace with trait alias.
/// The `App::state` will be cloned when a request inbounds.
///
/// `State` is designed to share data or handler between middlewares.
///
/// ### Example
/// ```rust
/// use roa_core::App;
/// use log::info;
/// use async_std::task::spawn;
/// use http::StatusCode;
///
/// #[derive(Clone)]
/// struct State {
///     id: u64,
/// }
///
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let (addr, server) = App::new(State { id: 0 })
///         .gate_fn(|mut ctx, next| async move {
///             ctx.id = 1;
///             next.await
///         })
///         .end(|ctx| async move {
///             let id = ctx.id;
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
pub trait State: 'static + Clone + Send + Sync + Sized {}

impl<T: 'static + Clone + Send + Sync + Sized> State for T {}
