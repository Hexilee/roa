use juniper::{
    http::GraphQLRequest, serde::Deserialize, Context as JuniperContext,
    DefaultScalarValue, GraphQLTypeAsync, InputValue, RootNode, ScalarValue,
};
use roa_body::PowerBody;

use futures::Future;
use roa_core::{async_trait, Context, Error, Middleware, Next, Request, Result, State};
use std::pin::Pin;
use std::sync::Arc;

pub struct GraphQL<QueryT, MutationT, Sca>(RootNode<'static, QueryT, MutationT, Sca>)
where
    Sca: 'static + ScalarValue + Send + Sync,
    QueryT: GraphQLTypeAsync<Sca> + Send + Sync + 'static,
    MutationT: GraphQLTypeAsync<Sca> + Send + Sync + 'static,
    QueryT::Context: Send + Sync + 'static,
    MutationT::Context: Send + Sync + 'static,
    QueryT::TypeInfo: Send + Sync,
    MutationT::TypeInfo: Send + Sync;

#[async_trait(?Send)]
impl<S, QueryT, MutationT, Ctx, Sca> Middleware<S> for GraphQL<QueryT, MutationT, Sca>
where
    S: State,
    Sca: 'static + ScalarValue + Send + Sync,
    Ctx: Send + Sync + 'static,
    QueryT: GraphQLTypeAsync<Sca, Context = Ctx> + Send + Sync + 'static,
    MutationT: GraphQLTypeAsync<Sca, Context = Ctx> + Send + Sync + 'static,
    QueryT::TypeInfo: Send + Sync,
    MutationT::TypeInfo: Send + Sync,
{
    async fn handle(self: Arc<Self>, mut ctx: Context<S>, next: Next) -> Result {
        let request: GraphQLRequest<Sca> = ctx.read_json().await?;
        Ok(())
    }
}
