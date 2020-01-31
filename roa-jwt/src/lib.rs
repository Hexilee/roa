pub use async_trait::async_trait;
pub use jsonwebtoken::Validation;

use http::header::{AUTHORIZATION, WWW_AUTHENTICATE};
use http::HeaderValue;
use jsonwebtoken::{dangerous_unsafe_decode, decode};
use roa_core::{Context, Model, Next, Status, StatusCode};
use serde::{de::DeserializeOwned, Serialize};

const INVALID_HEADER_VALUE: &str = r#"Bearer realm="<jwt>", error="invalid_token""#;

#[async_trait]
pub trait JwtVerifier<M, C>
where
    M: Model,
    C: 'static + Serialize + DeserializeOwned,
{
    fn get_validation(&self) -> Validation {
        Validation::default()
    }
    async fn get_secret(&self, claim: &C) -> Result<Vec<u8>, Status>;
    async fn set_claim(&mut self, claim: C);
}

fn unauthorized_error<M: Model>(ctx: &mut Context<M>, www_authentication: &'static str) -> Status {
    ctx.response.headers.insert(
        WWW_AUTHENTICATE,
        HeaderValue::from_static(www_authentication),
    );
    Status::new(StatusCode::UNAUTHORIZED, "".to_string(), false)
}

fn try_get_token<M: Model>(ctx: Context<M>) -> Result<String, Status> {
    match ctx.request.headers.get(AUTHORIZATION) {
        None => Err(unauthorized_error(&mut ctx.clone(), INVALID_HEADER_VALUE)),
        Some(value) => {
            let token = value
                .to_str()
                .map_err(|_| unauthorized_error(&mut ctx.clone(), INVALID_HEADER_VALUE))?;
            match token.find("Bearer") {
                None => Err(unauthorized_error(&mut ctx.clone(), INVALID_HEADER_VALUE)),
                Some(n) => Ok(token[n + 6..].trim().to_string()),
            }
        }
    }
}

pub async fn jwt_verify<M, C>(mut ctx: Context<M>, next: Next) -> Result<(), Status>
where
    M: Model,
    C: 'static + Serialize + DeserializeOwned,
    M::State: JwtVerifier<M, C>,
{
    let token = try_get_token(ctx.clone())?;
    let dangerous_data = dangerous_unsafe_decode(&token)
        .map_err(|_err| unauthorized_error(&mut ctx.clone(), INVALID_HEADER_VALUE))?;
    let secret = ctx.get_secret(&dangerous_data.claims).await?;
    let validation = ctx.get_validation();
    let token_data = decode(&token, &secret, &validation)
        .map_err(|_err| unauthorized_error(&mut ctx.clone(), INVALID_HEADER_VALUE))?;
    ctx.set_claim(token_data.claims).await;
    next().await
}

#[cfg(test)]
mod tests {
    use crate::{async_trait, jwt_verify, JwtVerifier, INVALID_HEADER_VALUE};
    use http::header::{AUTHORIZATION, WWW_AUTHENTICATE};
    use http::{HeaderValue, StatusCode};
    use jsonwebtoken::{encode, Header};
    use roa_core::{App, Model, Request, Status};
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

    struct AppModel {
        secret: Vec<u8>,
    }

    struct AppState {
        user: Option<User>,
        secret: Vec<u8>,
    }

    impl Model for AppModel {
        type State = AppState;
        fn new_state(&self) -> Self::State {
            AppState {
                user: None,
                secret: self.secret.clone(),
            }
        }
    }

    #[async_trait]
    impl JwtVerifier<AppModel, User> for AppState {
        async fn get_secret(&self, _claim: &User) -> Result<Vec<u8>, Status> {
            Ok(self.secret.clone())
        }

        async fn set_claim(&mut self, claim: User) {
            self.user = Some(claim)
        }
    }

    const SECRET: &[u8] = b"123456";

    #[tokio::test]
    async fn verify() -> Result<(), Box<dyn std::error::Error>> {
        let mut app = App::new(AppModel {
            secret: SECRET.to_vec(),
        });
        app.join(jwt_verify).join(move |ctx, _next| {
            async move {
                match ctx.user {
                    None => panic!("ctx.usr should not be None"),
                    Some(ref user) => {
                        assert_eq!(0, user.id);
                        assert_eq!("Hexilee", &user.name);
                    }
                }
                Ok(())
            }
        });
        let addr = "127.0.0.1:8000".parse()?;
        // no header value
        let resp = app.serve(Request::new(), addr).await?;
        assert_eq!(StatusCode::UNAUTHORIZED, resp.status);
        assert_eq!(
            INVALID_HEADER_VALUE,
            resp.headers[WWW_AUTHENTICATE].to_str()?
        );

        // non-string header value
        let mut req = Request::new();
        req.headers
            .insert(AUTHORIZATION, HeaderValue::from_bytes([255].as_ref())?);
        let resp = app.serve(req, addr).await?;
        assert_eq!(StatusCode::UNAUTHORIZED, resp.status);
        assert_eq!(
            INVALID_HEADER_VALUE,
            resp.headers[WWW_AUTHENTICATE].to_str()?
        );

        // non-Bearer header value
        let mut req = Request::new();
        req.headers
            .insert(AUTHORIZATION, HeaderValue::from_static("Basic hahaha"));
        let resp = app.serve(req, addr).await?;
        assert_eq!(StatusCode::UNAUTHORIZED, resp.status);
        assert_eq!(
            INVALID_HEADER_VALUE,
            resp.headers[WWW_AUTHENTICATE].to_str()?
        );

        // invalid token
        let mut req = Request::new();
        req.headers
            .insert(AUTHORIZATION, HeaderValue::from_static("Bearer hahaha"));
        let resp = app.serve(req, addr).await?;
        assert_eq!(StatusCode::UNAUTHORIZED, resp.status);
        assert_eq!(
            INVALID_HEADER_VALUE,
            resp.headers[WWW_AUTHENTICATE].to_str()?
        );

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
            format!("Bearer {}", encode(&Header::default(), &user, SECRET)?).parse()?,
        );
        let resp = app.serve(req, addr).await?;
        assert_eq!(StatusCode::UNAUTHORIZED, resp.status);
        assert_eq!(
            INVALID_HEADER_VALUE,
            resp.headers[WWW_AUTHENTICATE].to_str()?
        );

        let mut req = Request::new();
        user.exp = (SystemTime::now() + Duration::from_millis(60))
            .duration_since(UNIX_EPOCH)?
            .as_secs(); // one hour later
        req.headers.insert(
            AUTHORIZATION,
            format!("Bearer {}", encode(&Header::default(), &user, SECRET)?).parse()?,
        );
        let resp = app.serve(req, addr).await?;
        assert_eq!(StatusCode::OK, resp.status);
        Ok(())
    }
}
