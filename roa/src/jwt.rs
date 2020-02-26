//! The jwt module of roa.
//! This module provides middlewares `guard` and `guard_by`
//! and a context extension `JwtVerifier`.
//!
//! ### Example
//!
//! ```rust
//! use roa::jwt::guard;
//! use roa::App;
//! use roa::http::header::AUTHORIZATION;
//! use roa::http::StatusCode;
//! use roa::preload::*;
//! use async_std::task::spawn;
//! use jsonwebtoken::{encode, Header};
//! use serde::{Deserialize, Serialize};
//! use std::time::{Duration, SystemTime, UNIX_EPOCH};
//!
//! #[derive(Debug, Serialize, Deserialize)]
//! struct User {
//!     sub: String,
//!     company: String,
//!     exp: u64,
//!     id: u64,
//!     name: String,
//! }
//!
//! const SECRET: &str = "123456";
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let mut app = App::new(());
//!     let (addr, server) = app
//!         .gate(guard(SECRET))
//!         .end(move |ctx| async move {
//!             let user: User = ctx.claims()?;
//!             assert_eq!(0, user.id);
//!             assert_eq!("Hexilee", &user.name);
//!             Ok(())
//!         })
//!         .run_local()?;
//!     spawn(server);
//!     let mut user = User {
//!         sub: "user".to_string(),
//!         company: "None".to_string(),
//!         exp: (SystemTime::now() + Duration::from_secs(86400))
//!             .duration_since(UNIX_EPOCH)?
//!             .as_secs(), // one second ago
//!         id: 0,
//!         name: "Hexilee".to_string(),
//!     };
//!
//!     let client = reqwest::Client::new();
//!     let resp = client
//!         .get(&format!("http://{}", addr))
//!         .header(
//!             AUTHORIZATION,
//!             format!(
//!                 "Bearer {}",
//!                 encode(&Header::default(), &user, SECRET.as_bytes())?
//!             ),
//!         )
//!         .send()
//!         .await?;
//!     assert_eq!(StatusCode::OK, resp.status());
//!     Ok(())
//! }
//! ```

pub use jsonwebtoken::Validation;

use crate::http::header::{HeaderValue, AUTHORIZATION, WWW_AUTHENTICATE};
use crate::http::StatusCode;
use crate::{async_trait, join, Context, Error, Middleware, Next, Result, State};
use jsonwebtoken::{dangerous_unsafe_decode, decode};
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::sync::Arc;

const INVALID_TOKEN: &str = r#"Bearer realm="<jwt>", error="invalid_token""#;

struct JwtScope;

/// A context extension.
/// This extension must be used in downstream of middleware `guard` or `guard_by`,
/// otherwise you cannot get expected claims.
///
/// ### Example
///
/// ```rust
/// use roa::{Context, Result};
/// use roa::jwt::JwtVerifier;
/// use serde_json::Value;
///
/// async fn get(ctx: Context<()>) -> Result {
///     let claims: Value = ctx.claims()?;
///     Ok(())
/// }
/// ```
pub trait JwtVerifier<S, C>
where
    C: 'static + DeserializeOwned,
{
    /// Deserialize claims from token.
    fn claims(&self) -> Result<C>;

    /// Verify token and deserialize claims with a validation.
    /// Use this method if this validation is different from that one of guard or guard_by.
    fn verify(&self, validation: &Validation) -> Result<C>;
}

/// Guard by default validation.
pub fn guard<S: State>(secret: impl ToString) -> impl Middleware<S> {
    guard_by(secret, Validation::default())
}

/// Guard downstream.
///
/// The json web token should be deliver by request header "authorization",
/// in format of `Authorization: Bearer <token>`.
///
/// If request fails to pass verification, return 401 UNAUTHORIZED and set response header:
///
/// `WWW-Authenticate: Bearer realm="<jwt>", error="invalid_token"`.
pub fn guard_by<S: State>(
    secret: impl ToString,
    validation: Validation,
) -> impl Middleware<S> {
    join(
        Arc::new(catch_www_authenticate),
        JwtGuard {
            secret: Arc::new(secret.to_string()),
            validation,
        },
    )
}

async fn catch_www_authenticate<S: State>(mut ctx: Context<S>, next: Next) -> Result {
    let result = next.await;
    if let Err(ref err) = result {
        if err.status_code == StatusCode::UNAUTHORIZED {
            ctx.resp_mut()
                .headers
                .insert(WWW_AUTHENTICATE, HeaderValue::from_static(INVALID_TOKEN));
        }
    }
    result
}

struct JwtGuard {
    secret: Arc<String>,
    validation: Validation,
}

fn unauthorized(_err: impl ToString) -> Error {
    Error::new(StatusCode::UNAUTHORIZED, "".to_string(), false)
}

fn guard_not_set() -> Error {
    Error::new(
        StatusCode::INTERNAL_SERVER_ERROR,
        "middleware `JwtGuard` is not set correctly",
        false,
    )
}

fn try_get_token<S: State>(ctx: &Context<S>) -> Result<String> {
    match ctx.header(AUTHORIZATION) {
        None | Some(Err(_)) => Err(unauthorized("")),
        Some(Ok(value)) => match value.find("Bearer") {
            None => Err(unauthorized("")),
            Some(n) => Ok(value[n + 6..].trim().to_string()),
        },
    }
}

impl<S, C> JwtVerifier<S, C> for Context<S>
where
    S: State,
    C: 'static + DeserializeOwned + Send,
{
    fn claims(&self) -> Result<C> {
        let token = self.load_scoped::<JwtScope, String>("token");
        match token {
            Some(token) => dangerous_unsafe_decode(&*token)
                .map(|data| data.claims)
                .map_err(|err| {
                    Error::new(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!(
                            "{}\ntoken deserialized fails, this maybe a bug of JwtGuard.",
                            err
                        ),
                        false,
                    )
                }),
            None => Err(guard_not_set()),
        }
    }

    fn verify(&self, validation: &Validation) -> Result<C> {
        let secret = self.load_scoped::<JwtScope, Arc<String>>("secret");
        let token = self.load_scoped::<JwtScope, String>("token");
        match (secret, token) {
            (Some(secret), Some(token)) => decode(&token, secret.as_bytes(), validation)
                .map(|data| data.claims)
                .map_err(unauthorized),
            _ => Err(guard_not_set()),
        }
    }
}

#[async_trait(?Send)]
impl<S: State> Middleware<S> for JwtGuard {
    async fn handle(self: Arc<Self>, mut ctx: Context<S>, next: Next) -> Result {
        let token = try_get_token(&ctx)?;
        decode::<Value>(&token, self.secret.as_bytes(), &self.validation)
            .map_err(unauthorized)?;
        ctx.store_scoped(JwtScope, "secret", self.secret.clone());
        ctx.store_scoped(JwtScope, "token", token);
        next.await
    }
}

#[cfg(test)]
mod tests {
    use super::{guard, INVALID_TOKEN};
    use crate::http::header::{AUTHORIZATION, WWW_AUTHENTICATE};
    use crate::http::StatusCode;
    use crate::preload::*;
    use crate::{App, Error};
    use async_std::task::spawn;
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

    #[tokio::test]
    async fn claims() -> Result<(), Box<dyn std::error::Error>> {
        let mut app = App::new(());
        let (addr, server) = app
            .gate(guard(SECRET))
            .end(move |ctx| async move {
                let user: User = ctx.claims()?;
                assert_eq!(0, user.id);
                assert_eq!("Hexilee", &user.name);
                Ok(())
            })
            .run_local()?;
        spawn(server);
        let resp = reqwest::get(&format!("http://{}", addr)).await?;
        assert_eq!(StatusCode::UNAUTHORIZED, resp.status());
        assert_eq!(INVALID_TOKEN, resp.headers()[WWW_AUTHENTICATE].to_str()?);

        // non-string header value
        let client = reqwest::Client::new();
        let resp = client
            .get(&format!("http://{}", addr))
            .header(AUTHORIZATION, [255].as_ref())
            .send()
            .await?;
        assert_eq!(StatusCode::UNAUTHORIZED, resp.status());
        assert_eq!(INVALID_TOKEN, resp.headers()[WWW_AUTHENTICATE].to_str()?);

        // non-Bearer header value
        let resp = client
            .get(&format!("http://{}", addr))
            .header(AUTHORIZATION, "Basic hahaha")
            .send()
            .await?;
        assert_eq!(StatusCode::UNAUTHORIZED, resp.status());
        assert_eq!(INVALID_TOKEN, resp.headers()[WWW_AUTHENTICATE].to_str()?);

        // invalid token
        let resp = client
            .get(&format!("http://{}", addr))
            .header(AUTHORIZATION, "Bearer hahaha")
            .send()
            .await?;
        assert_eq!(StatusCode::UNAUTHORIZED, resp.status());
        assert_eq!(INVALID_TOKEN, resp.headers()[WWW_AUTHENTICATE].to_str()?);

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
        let resp = client
            .get(&format!("http://{}", addr))
            .header(
                AUTHORIZATION,
                format!(
                    "Bearer {}",
                    encode(&Header::default(), &user, SECRET.as_bytes())?
                ),
            )
            .send()
            .await?;
        assert_eq!(StatusCode::UNAUTHORIZED, resp.status());
        assert_eq!(INVALID_TOKEN, resp.headers()[WWW_AUTHENTICATE].to_str()?);

        user.exp = (SystemTime::now() + Duration::from_millis(60))
            .duration_since(UNIX_EPOCH)?
            .as_secs(); // one hour later
        let resp = client
            .get(&format!("http://{}", addr))
            .header(
                AUTHORIZATION,
                format!(
                    "Bearer {}",
                    encode(&Header::default(), &user, SECRET.as_bytes())?
                ),
            )
            .send()
            .await?;
        assert_eq!(StatusCode::OK, resp.status());
        Ok(())
    }

    #[tokio::test]
    async fn jwt_verify_not_set() -> Result<(), Box<dyn std::error::Error>> {
        let mut app = App::new(());
        let (addr, server) = app
            .gate_fn(move |ctx, _next| async move {
                let result: Result<User, Error> = ctx.claims();
                assert!(result.is_err());
                let status = result.unwrap_err();
                assert_eq!(StatusCode::INTERNAL_SERVER_ERROR, status.status_code);
                assert_eq!("middleware `JwtGuard` is not set correctly", status.message);
                Ok(())
            })
            .run_local()?;
        spawn(server);
        reqwest::get(&format!("http://{}", addr)).await?;
        Ok(())
    }
}
