//!
//! ## roa::jsonrpc
//!
//! This module provides a json rpc endpoint.
//!
//! ### Example
//!
//! ```rust,no_run
//! use roa::App;
//! use roa::jsonrpc::{RpcEndpoint, Data, Error, Params, Server};
//! use roa::tcp::Listener;
//! use tracing::info;
//!
//! #[derive(serde::Deserialize)]
//! struct TwoNums {
//!     a: usize,
//!     b: usize,
//! }
//!
//! async fn add(Params(params): Params<TwoNums>) -> Result<usize, Error> {
//!     Ok(params.a + params.b)
//! }
//!
//! async fn sub(Params(params): Params<(usize, usize)>) -> Result<usize, Error> {
//!     Ok(params.0 - params.1)
//! }
//!
//! async fn message(data: Data<String>) -> Result<String, Error> {
//!     Ok(String::from(&*data))
//! }
//!
//! #[async_std::main]
//! async fn main() -> anyhow::Result<()> {
//!     let rpc = Server::new()
//!         .with_data(Data::new(String::from("Hello!")))
//!         .with_method("sub", sub)
//!         .with_method("message", message)
//!         .finish_unwrapped();
//!
//!     let app = App::new().end(RpcEndpoint(rpc));
//!     app.listen("127.0.0.1:8000", |addr| {
//!         info!("Server is listening on {}", addr)
//!     })?
//!     .await?;
//!     Ok(())
//! }
//! ```

use bytes::Bytes;
#[doc(no_inline)]
pub use jsonrpc_v2::*;

use crate::body::PowerBody;
use crate::{async_trait, Context, Endpoint, Result, State};

/// A wrapper for [`jsonrpc_v2::Server`], implemented [`roa::Endpoint`].
///
/// [`jsonrpc_v2::Server`]: https://docs.rs/jsonrpc-v2/0.10.1/jsonrpc_v2/struct.Server.html
/// [`roa::Endpoint`]: https://docs.rs/roa/0.6.0/roa/trait.Endpoint.html
pub struct RpcEndpoint<R>(pub Server<R>);

#[async_trait(? Send)]
impl<'a, S, R> Endpoint<'a, S> for RpcEndpoint<R>
where
    S: State,
    R: Router + Sync + Send + 'static,
{
    #[inline]
    async fn call(&'a self, ctx: &'a mut Context<S>) -> Result {
        let data = ctx.read().await?;
        let resp = self.0.handle(Bytes::from(data)).await;
        ctx.write_json(&resp)
    }
}
