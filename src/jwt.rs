pub use async_trait::async_trait;
pub use jsonwebtoken::Validation;

use crate::{Context, DynTargetHandler, Model, Next, Status, StatusCode, TargetHandler};
use http::header::{AUTHORIZATION, WWW_AUTHENTICATE};
use http::HeaderValue;
use jsonwebtoken::decode;
use serde::{de::DeserializeOwned, Serialize};

const INVALID_TOKEN: &str = r#"Bearer realm="<jwt>", error="invalid_token""#;

struct JwtSymbol;

#[async_trait]
pub trait JwtVerifier<M, C>
where
    M: Model,
    C: 'static + Serialize + DeserializeOwned,
{
    async fn verify(&self, validation: &Validation) -> Result<C, Status>;
}

async fn unauthorized_error<M: Model, E: ToString>(
    ctx: &Context<M>,
    www_authentication: &'static str,
) -> impl Fn(E) -> Status {
    ctx.resp_mut().await.headers.insert(
        WWW_AUTHENTICATE,
        HeaderValue::from_static(www_authentication),
    );
    |_err| Status::new(StatusCode::UNAUTHORIZED, "".to_string(), false)
}

async fn try_get_token<M: Model>(ctx: &Context<M>) -> Result<String, Status> {
    match ctx.header(&AUTHORIZATION).await {
        None | Some(Err(_)) => Err((unauthorized_error(&mut ctx.clone(), INVALID_TOKEN).await)(
            "",
        )),
        Some(Ok(value)) => match value.find("Bearer") {
            None => Err((unauthorized_error(&mut ctx.clone(), INVALID_TOKEN).await)(
                "",
            )),
            Some(n) => Ok(value[n + 6..].trim().to_string()),
        },
    }
}

pub fn jwt_verify<M, C>(secret: String, validation: Validation) -> Box<DynTargetHandler<M, Next>>
where
    M: Model,
    C: 'static + Serialize + DeserializeOwned + Send,
{
    Box::new(move |ctx, next: Next| {
        let secret = secret.clone();
        let validation = validation.clone();
        async move {
            let token = try_get_token(&ctx).await?;
            decode::<C>(&token, secret.as_bytes(), &validation)
                .map_err(unauthorized_error(&ctx, INVALID_TOKEN).await)?;
            ctx.store::<JwtSymbol>("secret", secret).await;
            ctx.store::<JwtSymbol>("token", token).await;
            next().await
        }
    })
    .dynamic()
}

#[async_trait]
impl<M, C> JwtVerifier<M, C> for Context<M>
where
    M: Model,
    C: 'static + Serialize + DeserializeOwned + Send,
{
    async fn verify(&self, validation: &Validation) -> Result<C, Status> {
        let secret = self.load::<JwtSymbol>("secret").await;
        let token = self.load::<JwtSymbol>("token").await;
        match (secret, token) {
            (Some(secret), Some(token)) => decode(&token, secret.as_bytes(), &validation)
                .map_err(unauthorized_error(self, INVALID_TOKEN).await)
                .map(|data| data.claims),
            _ => Err(Status::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "middleware `jwt_verify` is not set correctly",
                false,
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{jwt_verify, JwtVerifier, Validation, INVALID_TOKEN};
    use crate::{App, Request};
    use http::header::{AUTHORIZATION, WWW_AUTHENTICATE};
    use http::{HeaderValue, StatusCode};
    use jsonwebtoken::{encode, Header};
    use serde::{Deserialize, Serialize};
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    #[derive(Debug, Serialize, Deserialize)]
    struct User {
        sub: String,
        company: String,
        exp: u64,
        id: u64,
        name: String,
    }

    const SECRET: &str = "123456";

    #[async_std::test]
    async fn verify() -> Result<(), Box<dyn std::error::Error>> {
        let mut app = App::new(());
        app.gate(jwt_verify::<(), User>(
            SECRET.to_string(),
            Validation::default(),
        ))
        .gate(move |ctx, _next| async move {
            let user: User = ctx.verify(&Validation::default()).await?;
            assert_eq!(0, user.id);
            assert_eq!("Hexilee", &user.name);
            Ok(())
        });
        let addr = "127.0.0.1:8000".parse()?;
        // no header value
        let resp = app.serve(Request::new(), addr).await?;
        assert_eq!(StatusCode::UNAUTHORIZED, resp.status);
        assert_eq!(INVALID_TOKEN, resp.headers[WWW_AUTHENTICATE].to_str()?);

        // non-string header value
        let mut req = Request::new();
        req.headers
            .insert(AUTHORIZATION, HeaderValue::from_bytes([255].as_ref())?);
        let resp = app.serve(req, addr).await?;
        assert_eq!(StatusCode::UNAUTHORIZED, resp.status);
        assert_eq!(INVALID_TOKEN, resp.headers[WWW_AUTHENTICATE].to_str()?);

        // non-Bearer header value
        let mut req = Request::new();
        req.headers
            .insert(AUTHORIZATION, HeaderValue::from_static("Basic hahaha"));
        let resp = app.serve(req, addr).await?;
        assert_eq!(StatusCode::UNAUTHORIZED, resp.status);
        assert_eq!(INVALID_TOKEN, resp.headers[WWW_AUTHENTICATE].to_str()?);

        // invalid token
        let mut req = Request::new();
        req.headers
            .insert(AUTHORIZATION, HeaderValue::from_static("Bearer hahaha"));
        let resp = app.serve(req, addr).await?;
        assert_eq!(StatusCode::UNAUTHORIZED, resp.status);
        assert_eq!(INVALID_TOKEN, resp.headers[WWW_AUTHENTICATE].to_str()?);

        // expired token
        let mut user = User {
            sub: "user".to_string(),
            company: "None".to_string(),
            exp: (SystemTime::now() - Duration::from_secs(1))
                .duration_since(UNIX_EPOCH)?
                .as_secs(), // one second ago
            id: 0,
            name: "Hexilee".to_string(),
        };
        let mut req = Request::new();
        req.headers.insert(
            AUTHORIZATION,
            format!(
                "Bearer {}",
                encode(&Header::default(), &user, SECRET.as_bytes())?
            )
            .parse()?,
        );
        let resp = app.serve(req, addr).await?;
        assert_eq!(StatusCode::UNAUTHORIZED, resp.status);
        assert_eq!(INVALID_TOKEN, resp.headers[WWW_AUTHENTICATE].to_str()?);

        let mut req = Request::new();
        user.exp = (SystemTime::now() + Duration::from_millis(60))
            .duration_since(UNIX_EPOCH)?
            .as_secs(); // one hour later
        req.headers.insert(
            AUTHORIZATION,
            format!(
                "Bearer {}",
                encode(&Header::default(), &user, SECRET.as_bytes())?
            )
            .parse()?,
        );
        let resp = app.serve(req, addr).await?;
        assert_eq!(StatusCode::OK, resp.status);
        Ok(())
    }
}
