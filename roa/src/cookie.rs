//! This module provides a middleware `cookie_parser` and context extensions `CookieGetter` and `CookieSetter`.
//!
//! ### Example
//!
//! ```rust
//! use roa::cookie::cookie_parser;
//! use roa::preload::*;
//! use roa::{App, Context};
//! use std::error::Error;
//!
//! async fn end(ctx: &mut Context) -> roa::Result {
//!     assert_eq!("Hexilee", ctx.must_cookie("name")?.value());
//!     Ok(())
//! }
//!
//! # fn main() -> Result<(), Box<dyn Error>> {
//! let app = App::new(()).gate(cookie_parser).end(end);
//! let (addr, server) = app.run()?;
//! // server.await
//! Ok(())
//! # }
//! ```

use crate::header::FriendlyHeaders;
use crate::http::{header, StatusCode};
use crate::{throw, Context, Next, Result};
pub use cookie::Cookie;
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use std::sync::Arc;

/// A scope to store and load variables in Context::storage.
struct CookieScope;

/// A context extension.
/// This extension must be used in downstream of middleware `cookier_parser`,
/// otherwise you cannot get expected cookie.
///
/// Percent-encoded cookies will be decoded.
/// ### Example
///
/// ```rust
/// use roa::cookie::cookie_parser;
/// use roa::preload::*;
/// use roa::{App, Context};
/// use std::error::Error;
///
/// async fn end(ctx: &mut Context) -> roa::Result {
///     assert_eq!("Hexilee", ctx.must_cookie("name")?.value());
///     Ok(())
/// }
///
/// # fn main() -> Result<(), Box<dyn Error>> {
/// let app = App::new(()).gate(cookie_parser).end(end);
/// let (addr, server) = app.run()?;
/// // server.await
/// Ok(())
/// # }
/// ```
pub trait CookieGetter {
    /// Must get a cookie, throw 401 UNAUTHORIZED if it not exists.
    fn must_cookie(&mut self, name: &str) -> Result<Arc<Cookie<'static>>>;

    /// Try to get a cookie, return `None` if it not exists.
    ///
    /// ### Example
    ///
    /// ```rust
    /// use roa::cookie::cookie_parser;
    /// use roa::preload::*;
    /// use roa::{App, Context};
    /// use std::error::Error;
    ///
    /// async fn end(ctx: &mut Context) -> roa::Result {
    ///     assert!(ctx.cookie("name").is_none());
    ///     Ok(())
    /// }
    ///
    /// # fn main() -> Result<(), Box<dyn Error>> {
    /// let app = App::new(()).gate(cookie_parser).end(end);
    /// let (addr, server) = app.run()?;
    /// // server.await
    /// Ok(())
    /// # }
    /// ```
    fn cookie(&self, name: &str) -> Option<Arc<Cookie<'static>>>;
}

/// An extension to set cookie.
pub trait CookieSetter {
    /// Set a cookie in pecent encoding, should not return Err.
    /// ### Example
    ///
    /// ```rust
    /// use roa::cookie::{cookie_parser, Cookie};
    /// use roa::preload::*;
    /// use roa::{App, Context};
    /// use std::error::Error;
    ///
    /// async fn end(ctx: &mut Context) -> roa::Result {
    ///     ctx.set_cookie(Cookie::new("name", "Hexilee"));
    ///     Ok(())
    /// }
    ///
    /// # fn main() -> Result<(), Box<dyn Error>> {
    /// let app = App::new(()).gate(cookie_parser).end(end);
    /// let (addr, server) = app.run()?;
    /// // server.await
    /// Ok(())
    /// # }
    /// ```
    fn set_cookie(&mut self, cookie: Cookie<'_>) -> Result;
}

/// A middleware to parse cookie.
#[inline]
pub async fn cookie_parser<S>(ctx: &mut Context<S>, next: Next<'_>) -> Result {
    if let Some(Ok(cookies)) = ctx.header(header::COOKIE) {
        for cookie in cookies
            .split(';')
            .map(|cookie| cookie.trim())
            .map(Cookie::parse_encoded)
            .filter_map(|cookie| cookie.ok())
            .map(|cookie| cookie.into_owned())
            .collect::<Vec<_>>()
            .into_iter()
        {
            let name = cookie.name().to_string();
            ctx.store_scoped(CookieScope, name, cookie);
        }
    }
    next.await
}

impl<S> CookieGetter for Context<S> {
    #[inline]
    fn must_cookie(&mut self, name: &str) -> Result<Arc<Cookie<'static>>> {
        match self.cookie(name) {
            Some(value) => Ok(value),
            None => {
                self.resp.insert(
                    header::WWW_AUTHENTICATE,
                    format!(
                        r#"Cookie name="{}""#,
                        utf8_percent_encode(name, NON_ALPHANUMERIC)
                    ),
                )?;
                throw!(StatusCode::UNAUTHORIZED)
            }
        }
    }

    #[inline]
    fn cookie(&self, name: &str) -> Option<Arc<Cookie<'static>>> {
        Some(self.load_scoped::<CookieScope, Cookie>(name)?.value())
    }
}

impl<S> CookieSetter for Context<S> {
    #[inline]
    fn set_cookie(&mut self, cookie: Cookie<'_>) -> Result {
        let cookie_value = cookie.encoded().to_string();
        self.resp.append(header::SET_COOKIE, cookie_value)?;
        Ok(())
    }
}

#[cfg(all(test, feature = "tcp"))]
mod tests {
    use crate::cookie::{cookie_parser, Cookie};
    use crate::http::{
        header::{COOKIE, WWW_AUTHENTICATE},
        StatusCode,
    };
    use crate::preload::*;
    use crate::{App, Context};
    use async_std::task::spawn;

    async fn must(ctx: &mut Context) -> crate::Result {
        assert_eq!("Hexi Lee", ctx.must_cookie("nick name")?.value());
        Ok(())
    }

    async fn none(ctx: &mut Context) -> crate::Result {
        assert!(ctx.cookie("nick name").is_none());
        Ok(())
    }

    #[tokio::test]
    async fn parser() -> Result<(), Box<dyn std::error::Error>> {
        // downstream of `cookie_parser`
        let (addr, server) = App::new(()).gate(cookie_parser).end(must).run()?;
        spawn(server);
        let client = reqwest::Client::new();
        let resp = client
            .get(&format!("http://{}", addr))
            .header(COOKIE, "nick%20name=Hexi%20Lee")
            .send()
            .await?;
        assert_eq!(StatusCode::OK, resp.status());

        // miss `cookie_parser`
        let (addr, server) = App::new(()).end(must).run()?;
        spawn(server);
        let resp = client
            .get(&format!("http://{}", addr))
            .header(COOKIE, "nick%20name=Hexi%20Lee")
            .send()
            .await?;
        assert_eq!(StatusCode::UNAUTHORIZED, resp.status());
        Ok(())
    }

    #[tokio::test]
    async fn cookie() -> Result<(), Box<dyn std::error::Error>> {
        // miss cookie
        let (addr, server) = App::new(()).end(none).run()?;
        spawn(server);
        let resp = reqwest::get(&format!("http://{}", addr)).await?;
        assert_eq!(StatusCode::OK, resp.status());

        let (addr, server) = App::new(()).gate(cookie_parser).end(must).run()?;
        spawn(server);
        let resp = reqwest::get(&format!("http://{}", addr)).await?;
        assert_eq!(StatusCode::UNAUTHORIZED, resp.status());
        assert_eq!(
            r#"Cookie name="nick%20name""#,
            resp.headers()
                .get(WWW_AUTHENTICATE)
                .unwrap()
                .to_str()
                .unwrap()
        );

        // string value
        let (addr, server) = App::new(()).gate(cookie_parser).end(must).run()?;
        spawn(server);
        let client = reqwest::Client::new();
        let resp = client
            .get(&format!("http://{}", addr))
            .header(COOKIE, "nick%20name=Hexi%20Lee")
            .send()
            .await?;
        assert_eq!(StatusCode::OK, resp.status());
        Ok(())
    }

    #[tokio::test]
    async fn cookie_action() -> Result<(), Box<dyn std::error::Error>> {
        async fn test(ctx: &mut Context) -> crate::Result {
            assert_eq!("bar baz", ctx.must_cookie("bar baz")?.value());
            assert_eq!("bar foo", ctx.must_cookie("foo baz")?.value());
            Ok(())
        }

        let (addr, server) = App::new(()).gate(cookie_parser).end(test).run()?;
        spawn(server);
        let client = reqwest::Client::new();
        let resp = client
            .get(&format!("http://{}", addr))
            .header(COOKIE, "bar%20baz=bar%20baz; foo%20baz=bar%20foo")
            .send()
            .await?;
        assert_eq!(StatusCode::OK, resp.status());
        Ok(())
    }

    #[tokio::test]
    async fn set_cookie() -> Result<(), Box<dyn std::error::Error>> {
        async fn test(ctx: &mut Context) -> crate::Result {
            ctx.set_cookie(Cookie::new("bar baz", "bar baz"))?;
            ctx.set_cookie(Cookie::new("bar foo", "foo baz"))?;
            Ok(())
        }
        let (addr, server) = App::new(()).end(test).run()?;
        spawn(server);
        let resp = reqwest::get(&format!("http://{}", addr)).await?;
        assert_eq!(StatusCode::OK, resp.status());
        let cookies: Vec<reqwest::cookie::Cookie> = resp.cookies().collect();
        assert_eq!(2, cookies.len());
        assert_eq!(("bar%20baz"), cookies[0].name());
        assert_eq!(("bar%20baz"), cookies[0].value());
        assert_eq!(("bar%20foo"), cookies[1].name());
        assert_eq!(("foo%20baz"), cookies[1].value());
        Ok(())
    }
}
