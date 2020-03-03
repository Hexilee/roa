//! This crate provides a juniper context and a graphql endpoint.
//!
//! ### Example
//!
//! Refer to [integration-example](https://github.com/Hexilee/roa/tree/master/integration/juniper-example)

#![warn(missing_docs)]

use juniper::{http::GraphQLRequest, GraphQLTypeAsync, RootNode, ScalarValue};
use roa_body::PowerBody;
use roa_core::{async_trait, Context, Middleware, Next, Result, State, SyncContext};
use std::ops::{Deref, DerefMut};
use std::sync::Arc;

/// A wrapper for `roa_core::SyncContext`.
/// As an implementation of `juniper::Context`.
pub struct JuniperContext<S>(SyncContext<S>);

impl<S: State> juniper::Context for JuniperContext<S> {}

impl<S> Deref for JuniperContext<S> {
    type Target = SyncContext<S>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<S> DerefMut for JuniperContext<S> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// An endpoint.
pub struct GraphQL<QueryT, MutationT, Sca>(
    pub RootNode<'static, QueryT, MutationT, Sca>,
)
where
    Sca: 'static + ScalarValue + Send + Sync,
    QueryT: GraphQLTypeAsync<Sca> + Send + Sync + 'static,
    MutationT: GraphQLTypeAsync<Sca> + Send + Sync + 'static,
    QueryT::Context: Send + Sync + 'static,
    MutationT::Context: Send + Sync + 'static,
    QueryT::TypeInfo: Send + Sync,
    MutationT::TypeInfo: Send + Sync;

#[async_trait(?Send)]
impl<S, QueryT, MutationT, Sca> Middleware<S> for GraphQL<QueryT, MutationT, Sca>
where
    S: State,
    Sca: 'static + ScalarValue + Send + Sync,
    QueryT: GraphQLTypeAsync<Sca, Context = JuniperContext<S>> + Send + Sync + 'static,
    MutationT:
        GraphQLTypeAsync<Sca, Context = JuniperContext<S>> + Send + Sync + 'static,
    QueryT::TypeInfo: Send + Sync,
    MutationT::TypeInfo: Send + Sync,
{
    async fn handle(self: Arc<Self>, mut ctx: Context<S>, _next: Next) -> Result {
        let request: GraphQLRequest<Sca> = ctx.read_json().await?;
        let juniper_ctx = JuniperContext(ctx.clone());
        let resp = request.execute_async(&self.0, &juniper_ctx).await;
        ctx.write_json(&resp)
    }
}
