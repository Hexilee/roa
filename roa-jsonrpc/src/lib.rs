#![cfg_attr(feature = "docs", feature(doc_cfg, external_doc))]
#![cfg_attr(feature = "docs", doc(include = "../README.md"))]
#![cfg_attr(feature = "docs", warn(missing_docs))]

use bytes::Bytes;
use roa::preload::*;
use roa::{async_trait, Context, Endpoint, Result, State};

pub use jsonrpc_v2::*;

/// A wrapper for [`jsonrpc_v2::Server`], implemented [`roa::Endpoint`].
///
/// [`jsonrpc_v2::Server`]: https://docs.rs/jsonrpc-v2/0.5.2/jsonrpc_v2/struct.Server.html
/// [`roa::Endpoint`]: https://docs.rs/roa/0.5.2/roa/trait.Endpoint.html
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
