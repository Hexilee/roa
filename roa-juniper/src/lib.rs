//! This crate provides a juniper context and a graphql endpoint.
//!
//! ### Example
//!
//! Refer to [integration-example](https://github.com/Hexilee/roa/tree/master/integration/juniper-example)

#![warn(missing_docs)]

use std::ops::{Deref, DerefMut};

use juniper::http::GraphQLRequest;
use juniper::{GraphQLType, GraphQLTypeAsync, RootNode, ScalarValue};
use roa::preload::*;
use roa::{async_trait, Context, Endpoint, Result, State};

/// A wrapper for `roa_core::SyncContext`.
/// As an implementation of `juniper::Context`.
pub struct JuniperContext<S>(Context<S>);

impl<S: State> juniper::Context for JuniperContext<S> {}

impl<S> Deref for JuniperContext<S> {
    type Target = Context<S>;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<S> DerefMut for JuniperContext<S> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// An endpoint.
pub struct GraphQL<QueryT, MutationT, SubscriptionT, Sca>(
    pub RootNode<'static, QueryT, MutationT, SubscriptionT, Sca>,
)
where
    QueryT: GraphQLType<Sca>,
    MutationT: GraphQLType<Sca>,
    SubscriptionT: GraphQLType<Sca>,
    Sca: ScalarValue;

#[async_trait(?Send)]
impl<'a, S, QueryT, MutationT, SubscriptionT, Sca> Endpoint<'a, S>
    for GraphQL<QueryT, MutationT, SubscriptionT, Sca>
where
    S: State,
    QueryT: GraphQLTypeAsync<Sca, Context = JuniperContext<S>> + Send + Sync + 'static,
    QueryT::TypeInfo: Send + Sync,
    MutationT: GraphQLTypeAsync<Sca, Context = QueryT::Context> + Send + Sync + 'static,
    MutationT::TypeInfo: Send + Sync,
    SubscriptionT: GraphQLType<Sca, Context = QueryT::Context> + Send + Sync + 'static,
    SubscriptionT::TypeInfo: Send + Sync,
    Sca: ScalarValue + Send + Sync + 'static,
{
    #[inline]
    async fn call(&'a self, ctx: &'a mut Context<S>) -> Result {
        let request: GraphQLRequest<Sca> = ctx.read_json().await?;
        let juniper_ctx = JuniperContext(ctx.clone());
        let resp = request.execute(&self.0, &juniper_ctx).await;
        ctx.write_json(&resp)
    }
}
