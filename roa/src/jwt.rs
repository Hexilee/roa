//! This module provides middleware `JwtGuard` and a context extension `JwtVerifier`.
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
//! async fn test(ctx: &mut Context) -> roa::Result {
//!     let user: User = ctx.claims()?;
//!     assert_eq!(0, user.id);
//!     assert_eq!("Hexilee", &user.name);
//!     Ok(())
//! }
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let (addr, server) = App::new()
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

use headers::authorization::Bearer;
use headers::{Authorization, HeaderMapExt};
use jsonwebtoken::decode;
pub use jsonwebtoken::{DecodingKey, Validation};
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::http::header::{HeaderValue, WWW_AUTHENTICATE};
use crate::http::StatusCode;
use crate::{async_trait, throw, Context, Middleware, Next, Result, Status};

/// A private scope.
struct JwtScope;

static INVALID_TOKEN: HeaderValue =
    HeaderValue::from_static(r#"Bearer realm="<jwt>", error="invalid_token""#);

/// A function to set value of WWW_AUTHENTICATE.
#[inline]
fn set_www_authenticate<S>(ctx: &mut Context<S>) {
    ctx.resp
        .headers
        .insert(WWW_AUTHENTICATE, INVALID_TOKEN.clone());
}

/// Throw a internal server error.
#[inline]
fn guard_not_set() -> Status {
    Status::new(
        StatusCode::INTERNAL_SERVER_ERROR,
        "middleware `JwtGuard` is not set correctly",
        false,
    )
}

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
/// async fn get(ctx: &mut Context) -> Result {
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
    /// Use this method if this validation is different from that one of `JwtGuard`.
    fn verify<C>(&mut self, validation: &Validation) -> Result<C>
    where
        C: 'static + DeserializeOwned;
}

/// Guard by default validation.
pub fn guard(secret: DecodingKey) -> JwtGuard {
    JwtGuard::new(secret, Validation::default())
}

/// A middleware to deny unauthorized requests.
///
/// The json web token should be deliver by request header "authorization",
/// in format of `Authorization: Bearer <token>`.
///
/// If request fails to pass verification, return 401 UNAUTHORIZED and set response header "WWW-Authenticate".
#[derive(Debug, Clone, PartialEq)]
pub struct JwtGuard {
    secret: DecodingKey<'static>,
    validation: Validation,
}

impl JwtGuard {
    /// Construct guard.
    pub fn new(secret: DecodingKey, validation: Validation) -> Self {
        Self {
            secret: secret.into_static(),
            validation,
        }
    }

    /// Verify token.
    #[inline]
    fn verify<S>(&self, ctx: &Context<S>) -> Option<(Bearer, Value)> {
        let bearer = ctx.req.headers.typed_get::<Authorization<Bearer>>()?.0;
        let value = decode::<Value>(bearer.token(), &self.secret, &self.validation)
            .ok()?
            .claims;
        Some((bearer, value))
    }
}

#[async_trait(? Send)]
impl<'a, S> Middleware<'a, S> for JwtGuard {
    #[inline]
    async fn handle(&'a self, ctx: &'a mut Context<S>, next: Next<'a>) -> Result {
        match self.verify(ctx) {
            None => {
                set_www_authenticate(ctx);
                throw!(StatusCode::UNAUTHORIZED)
            }
            Some((bearer, value)) => {
                ctx.store_scoped(JwtScope, "secret", self.secret.clone());
                ctx.store_scoped(JwtScope, "token", bearer);
                ctx.store_scoped(JwtScope, "value", value);
                next.await
            }
        }
    }
}

impl<S> JwtVerifier<S> for Context<S> {
    #[inline]
    fn claims<C>(&self) -> Result<C>
    where
        C: 'static + DeserializeOwned,
    {
        let value = self.load_scoped::<JwtScope, Value>("value");
        match value {
            Some(claims) => Ok(serde_json::from_value((*claims).clone())?),
            None => Err(guard_not_set()),
        }
    }

    #[inline]
    fn verify<C>(&mut self, validation: &Validation) -> Result<C>
    where
        C: 'static + DeserializeOwned,
    {
        let secret = self.load_scoped::<JwtScope, DecodingKey<'static>>("secret");
        let token = self.load_scoped::<JwtScope, Bearer>("token");
        match (secret, token) {
            (Some(secret), Some(token)) => match decode(token.token(), &secret, validation) {
                Ok(data) => Ok(data.claims),
                Err(_) => {
                    set_www_authenticate(self);
                    throw!(StatusCode::UNAUTHORIZED)
                }
            },
            _ => Err(guard_not_set()),
        }
    }
}

#[cfg(all(test, feature = "tcp"))]
mod tests {
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use async_std::task::spawn;
    use jsonwebtoken::{encode, EncodingKey, Header};
    use serde::{Deserialize, Serialize};

    use super::{guard, DecodingKey, INVALID_TOKEN};
    use crate::http::header::{AUTHORIZATION, WWW_AUTHENTICATE};
    use crate::http::StatusCode;
    use crate::preload::*;
    use crate::{App, Context};

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
        async fn test(ctx: &mut Context) -> crate::Result {
            let user: User = ctx.claims()?;
            assert_eq!(0, user.id);
            assert_eq!("Hexilee", &user.name);
            Ok(())
        }
        let (addr, server) = App::new()
            .gate(guard(DecodingKey::from_secret(SECRET)))
            .end(test)
            .run()?;
        spawn(server);
        let resp = reqwest::get(&format!("http://{}", addr)).await?;
        assert_eq!(StatusCode::UNAUTHORIZED, resp.status());
        assert_eq!(&INVALID_TOKEN, &resp.headers()[WWW_AUTHENTICATE]);

        // non-string header value
        let client = reqwest::Client::new();
        let resp = client
            .get(&format!("http://{}", addr))
            .header(AUTHORIZATION, [255].as_ref())
            .send()
            .await?;
        assert_eq!(StatusCode::UNAUTHORIZED, resp.status());
        assert_eq!(&INVALID_TOKEN, &resp.headers()[WWW_AUTHENTICATE]);

        // non-Bearer header value
        let resp = client
            .get(&format!("http://{}", addr))
            .header(AUTHORIZATION, "Basic hahaha")
            .send()
            .await?;
        assert_eq!(StatusCode::UNAUTHORIZED, resp.status());
        assert_eq!(&INVALID_TOKEN, &resp.headers()[WWW_AUTHENTICATE]);

        // invalid token
        let resp = client
            .get(&format!("http://{}", addr))
            .header(AUTHORIZATION, "Bearer hahaha")
            .send()
            .await?;
        assert_eq!(StatusCode::UNAUTHORIZED, resp.status());
        assert_eq!(&INVALID_TOKEN, &resp.headers()[WWW_AUTHENTICATE]);

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
                    encode(&Header::default(), &user, &EncodingKey::from_secret(SECRET),)?
                ),
            )
            .send()
            .await?;
        assert_eq!(StatusCode::UNAUTHORIZED, resp.status());
        assert_eq!(&INVALID_TOKEN, &resp.headers()[WWW_AUTHENTICATE]);

        user.exp = (SystemTime::now() + Duration::from_millis(60))
            .duration_since(UNIX_EPOCH)?
            .as_secs(); // one hour later
        let resp = client
            .get(&format!("http://{}", addr))
            .header(
                AUTHORIZATION,
                format!(
                    "Bearer {}",
                    encode(&Header::default(), &user, &EncodingKey::from_secret(SECRET),)?
                ),
            )
            .send()
            .await?;
        assert_eq!(StatusCode::OK, resp.status());
        Ok(())
    }

    #[tokio::test]
    async fn jwt_verify_not_set() -> Result<(), Box<dyn std::error::Error>> {
        async fn test(ctx: &mut Context) -> crate::Result {
            let _: User = ctx.claims()?;
            Ok(())
        }
        let (addr, server) = App::new().end(test).run()?;
        spawn(server);
        let resp = reqwest::get(&format!("http://{}", addr)).await?;
        assert_eq!(StatusCode::INTERNAL_SERVER_ERROR, resp.status());
        Ok(())
    }
}
