use jsonwebtoken::{dangerous_unsafe_decode, decode, Validation};
pub use jsonwebtoken::{encode, Algorithm, Header};
use roa_core::{
    Context, DynHandler, DynMiddleware, DynTargetHandler, Handler, Model, Next, Status, StatusCode,
    StatusFuture, TargetHandler,
};
use serde::{de::DeserializeOwned, Serialize};
use std::future::Future;
use std::sync::Arc;

pub struct JwtVerifier<M, C>
where
    M: Model,
    C: 'static + Serialize + DeserializeOwned,
{
    token_getter: Arc<DynHandler<M, String>>,
    validation_getter: Arc<DynHandler<M, Validation>>,
    secret_getter: Arc<DynTargetHandler<M, C, Vec<u8>>>,
    claim_setter: Arc<DynTargetHandler<M, C>>,
}

impl<M, C> JwtVerifier<M, C>
where
    M: Model,
    C: 'static + Serialize + DeserializeOwned,
{
    pub fn new<TG, TGF, SG, SGF>(token: TG, secret: SG) -> Self
    where
        TG: 'static + Send + Sync + Fn(Context<M>) -> TGF,
        TGF: 'static + Send + Future<Output = Result<String, Status>>,
        SG: 'static + Send + Sync + Fn(Context<M>, C) -> SGF,
        SGF: 'static + Send + Future<Output = Result<Vec<u8>, Status>>,
    {
        Self {
            token_getter: Arc::from(Box::new(token).dynamic()),
            secret_getter: Arc::from(Box::new(secret).dynamic()),
            validation_getter: Arc::from(
                Box::new(|_ctx| async { Ok(Validation::default()) }).dynamic(),
            ),
            claim_setter: Arc::from(Box::new(|_ctx, _claim| async { Ok(()) }).dynamic()),
        }
    }

    pub fn validation<VG, VGF>(&mut self, validation: VG) -> &mut Self
    where
        VG: 'static + Send + Sync + Fn(Context<M>) -> VGF,
        VGF: 'static + Send + Future<Output = Result<Validation, Status>>,
    {
        self.validation_getter = Arc::from(Box::new(validation).dynamic());
        self
    }
}

impl<M, C> Clone for JwtVerifier<M, C>
where
    M: Model,
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

impl<M, C> TargetHandler<M, Next> for JwtVerifier<M, C>
where
    M: Model,
    C: 'static + Serialize + DeserializeOwned + Send,
{
    type StatusFuture = StatusFuture;
    fn handle(&self, ctx: Context<M>, next: Next) -> Self::StatusFuture {
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

    fn dynamic(self: Box<Self>) -> Box<DynMiddleware<M>> {
        Box::new(move |ctx, next| self.handle(ctx, next))
    }
}

// TODO: test this module
