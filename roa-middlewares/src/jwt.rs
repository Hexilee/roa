use jsonwebtoken::{dangerous_unsafe_decode, decode, Validation};
pub use jsonwebtoken::{encode, Algorithm, Header};
use roa::{
    Context, DynHandler, DynMiddleware, DynTargetHandler, Next, State, Status, StatusCode,
    StatusFuture, TargetHandler,
};
use serde::{de::DeserializeOwned, Serialize};
use std::sync::Arc;

pub struct JwtVerifier<S, C>
where
    S: State,
    C: 'static + Serialize + DeserializeOwned,
{
    token_getter: Arc<DynHandler<S, String>>,
    validation_getter: Arc<DynHandler<S, Validation>>,
    secret_getter: Arc<DynTargetHandler<S, C, Vec<u8>>>,
    claim_setter: Arc<DynTargetHandler<S, C>>,
}

impl<S, C> Clone for JwtVerifier<S, C>
where
    S: State,
    C: 'static + Serialize + DeserializeOwned,
{
    fn clone(&self) -> Self {
        Self {
            token_getter: self.token_getter.clone(),
            validation_getter: self.validation_getter.clone(),
            secret_getter: self.secret_getter.clone(),
            claim_setter: self.claim_setter.clone(),
        }
    }
}

impl<S, C> TargetHandler<S, Next> for JwtVerifier<S, C>
where
    S: State,
    C: 'static + Serialize + DeserializeOwned + Send,
{
    type StatusFuture = StatusFuture;
    fn handle(&self, ctx: Context<S>, next: Next) -> Self::StatusFuture {
        let jwt = self.clone();
        Box::pin(async move {
            let token = (jwt.token_getter)(ctx.clone()).await?;
            let dangerous_claim: C = dangerous_unsafe_decode(&token)
                .map_err(|err| Status::new(StatusCode::BAD_REQUEST, err.to_string(), true))?
                .claims;
            let secret = (jwt.secret_getter)(ctx.clone(), dangerous_claim).await?;
            let validation = (jwt.validation_getter)(ctx.clone()).await?;
            let claim: C = decode(&token, &secret, &validation)
                .map_err(|err| Status::new(StatusCode::FORBIDDEN, err.to_string(), true))?
                .claims;
            (jwt.claim_setter)(ctx, claim).await?;
            next().await
        })
    }

    fn dynamic(self: Box<Self>) -> Box<DynMiddleware<S>> {
        Box::new(move |ctx, next| self.handle(ctx, next))
    }
}
