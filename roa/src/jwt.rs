//! The jwt module of roa.
//! This module provides middlewares `guard` and `guard_by`
//! and a context extension `JwtVerifier`.
//!
//! ### Example
//!
//! ```rust
//! use roa::jwt::{guard, DecodingKey};
//! use roa::{App, Context};
//! use roa::http::header::AUTHORIZATION;
//! use roa::http::StatusCode;
//! use roa::preload::*;
//! use async_std::task::spawn;
//! use jsonwebtoken::{encode, Header, EncodingKey};
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
//! const SECRET: &[u8] = b"123456";
//!
//! async fn test(ctx: &mut Context<()>) -> roa::Result {
//!     let user: User = ctx.claims()?;
//!     assert_eq!(0, user.id);
//!     assert_eq!("Hexilee", &user.name);
//!     Ok(())
//! }
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let (addr, server) = App::new(())
//!         .gate(guard(DecodingKey::from_secret(SECRET)))
//!         .end(test).run()?;
//!     spawn(server);
//!     let mut user = User {
//!         sub: "user".to_string(),
//!         company: "None".to_string(),
//!         exp: (SystemTime::now() + Duration::from_secs(86400))
//!             .duration_since(UNIX_EPOCH)?
//!             .as_secs(),
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
//!                 encode(
//!                     &Header::default(),
//!                     &user,
//!                     &EncodingKey::from_secret(SECRET)
//!                 )?
//!             ),
//!         )
//!         .send()
//!         .await?;
//!     assert_eq!(StatusCode::OK, resp.status());
//!     Ok(())
//! }
//! ```

pub use jsonwebtoken::{DecodingKey, Validation};

use crate::http::header::{HeaderValue, WWW_AUTHENTICATE};
use crate::http::StatusCode;
use crate::{async_trait, Context, Error, Middleware, MiddlewareExt, Next, Result};
use headers::{authorization::Bearer, Authorization, HeaderMapExt};
use jsonwebtoken::decode;
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
/// async fn get(ctx: &mut Context<()>) -> Result {
///     let claims: Value = ctx.claims()?;
///     Ok(())
/// }
/// ```
pub trait JwtVerifier<S> {
    /// Deserialize claims from token.
    fn claims<C>(&self) -> Result<C>
    where
        C: 'static + DeserializeOwned;

    /// Verify token and deserialize claims with a validation.
    /// Use this method if this validation is different from that one of guard or guard_by.
    fn verify<C>(&self, validation: &Validation) -> Result<C>
    where
        C: 'static + DeserializeOwned;
}

/// Guard by default validation.
pub fn guard<S: 'static>(secret: DecodingKey) -> impl for<'a> Middleware<'a, S> {
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
pub fn guard_by<S: 'static>(
    secret: DecodingKey,
    validation: Validation,
) -> impl for<'a> Middleware<'a, S> {
    catch_www_authenticate.chain(JwtGuard {
        secret: secret.into_static(),
        validation,
    })
}

#[inline]
async fn catch_www_authenticate<S>(ctx: &mut Context<S>, next: Next<'_>) -> Result {
    let result = next.await;
    if let Err(ref err) = result {
        if err.status_code == StatusCode::UNAUTHORIZED {
            ctx.resp
                .headers
                .insert(WWW_AUTHENTICATE, HeaderValue::from_static(INVALID_TOKEN));
        }
    }
    result
}

struct JwtGuard {
    secret: DecodingKey<'static>,
    validation: Validation,
}

#[inline]
fn unauthorized(_err: impl ToString) -> Error {
    Error::new(StatusCode::UNAUTHORIZED, "".to_string(), false)
}

#[inline]
fn guard_not_set() -> Error {
    Error::new(
        StatusCode::INTERNAL_SERVER_ERROR,
        "middleware `JwtGuard` is not set correctly",
        false,
    )
}

impl<S> JwtVerifier<S> for Context<S> {
    #[inline]
    fn claims<C>(&self) -> Result<C>
    where
        C: 'static + DeserializeOwned,
    {
        let value = self.load_scoped::<JwtScope, Value>("value");
        match value {
            Some(claims) => serde_json::from_value((*claims).clone())
                .map_err(|err| {
                    Error::new(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!(
                            "{}\nClaims value deserialized fails, this may be a bug of JwtGuard.",
                            err
                        ),
                        false,
                    )
                }),
            None => Err(guard_not_set()),
        }
    }

    #[inline]
    fn verify<C>(&self, validation: &Validation) -> Result<C>
    where
        C: 'static + DeserializeOwned,
    {
        let secret = self.load_scoped::<JwtScope, DecodingKey<'static>>("secret");
        let token = self.load_scoped::<JwtScope, Bearer>("token");
        match (secret, token) {
            (Some(secret), Some(token)) => decode(token.token(), &secret, validation)
                .map(|data| data.claims)
                .map_err(unauthorized),
            _ => Err(guard_not_set()),
        }
    }
}

#[async_trait(? Send)]
impl<'a, S> Middleware<'a, S> for JwtGuard {
    #[inline]
    async fn handle(&'a self, ctx: &'a mut Context<S>, next: Next<'a>) -> Result {
        let bearer = ctx
            .req
            .headers
            .typed_get::<Authorization<Bearer>>()
            .ok_or_else(|| unauthorized(""))?
            .0;
        let value = decode::<Value>(bearer.token(), &self.secret, &self.validation)
            .map_err(unauthorized)?;
        ctx.store_scoped(JwtScope, "secret", self.secret.clone());
        ctx.store_scoped(JwtScope, "token", bearer);
        ctx.store_scoped(JwtScope, "value", value.claims);
        next.await
    }
}

#[cfg(test)]
mod tests {
    use super::{guard, DecodingKey, INVALID_TOKEN};
    use crate::http::header::{AUTHORIZATION, WWW_AUTHENTICATE};
    use crate::http::StatusCode;
    use crate::preload::*;
    use crate::{App, Context, Error};
    use async_std::task::spawn;
    use jsonwebtoken::{encode, EncodingKey, Header};
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

    const SECRET: &[u8] = b"123456";

    #[tokio::test]
    async fn claims() -> Result<(), Box<dyn std::error::Error>> {
        async fn test(ctx: &mut Context<()>) -> crate::Result {
            let user: User = ctx.claims()?;
            assert_eq!(0, user.id);
            assert_eq!("Hexilee", &user.name);
            Ok(())
        }
        let (addr, server) = App::new(())
            .gate(guard(DecodingKey::from_secret(SECRET)))
            .end(test)
            .run()?;
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
                    encode(
                        &Header::default(),
                        &user,
                        &EncodingKey::from_secret(SECRET),
                    )?
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
                    encode(
                        &Header::default(),
                        &user,
                        &EncodingKey::from_secret(SECRET),
                    )?
                ),
            )
            .send()
            .await?;
        assert_eq!(StatusCode::OK, resp.status());
        Ok(())
    }

    // #[tokio::test]
    // async fn jwt_verify_not_set() -> Result<(), Box<dyn std::error::Error>> {
    //     async fn test(ctx: &mut Context<()>) -> crate::Result {
    //         let _: User = ctx.claims()?;
    //         Ok(())
    //     }
    //     let (addr, server) = App::new(()).end(test).run()?;
    //     spawn(server);
    //     let resp = reqwest::get(&format!("http://{}", addr)).await?;
    //     assert_eq!(StatusCode::INTERNAL_SERVER_ERROR, resp.status());
    //     assert_eq!(
    //         "middleware `JwtGuard` is not set correctly",
    //         resp.text().await?
    //     );
    //     Ok(())
    // }
}
