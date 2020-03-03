//! The cookie module of roa.
//! This module provides a middleware `cookie_parser` and a context extension `Cookier`.
//!
//! ### Example
//!
//! ```rust
//! use roa::cookie::cookie_parser;
//! use roa::preload::*;
//! use roa::App;
//! use roa::http::{StatusCode, header::COOKIE};
//! use async_std::task::spawn;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let (addr, server) = App::new(())
//!         .gate(cookie_parser)
//!         .end(|mut ctx| async move {
//!             assert_eq!("Hexilee", ctx.must_cookie("name")?.value());
//!             Ok(())
//!         })
//!         .run()?;
//!     spawn(server);
//!     let client = reqwest::Client::new();
//!     let resp = client
//!         .get(&format!("http://{}", addr))
//!         .header(COOKIE, "name=Hexilee")
//!         .send()
//!         .await?;
//!     assert_eq!(StatusCode::OK, resp.status());
//!     Ok(())
//! }
//! ```

use crate::header::FriendlyHeaders;
use crate::http::{header, StatusCode};
use crate::{throw, Context, Next, Result, State, SyncContext};
pub use cookie::Cookie;
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use std::sync::Arc;

const WWW_AUTHENTICATE_BUG_HELP: &str = "
Invalid WWW_AUTHENTICATE value, this is a bug of roa::cookie.
Please report it to https://github.com/Hexilee/roa.
";

/// A scope to store and load variables in Context::storage.
struct CookieScope;

/// A context extension.
/// This extension must be used in downstream of middleware `cookier_parser`,
/// otherwise you cannot get expected cookie.
///
/// ### Example
///
/// ```rust
/// use roa::cookie::cookie_parser;
/// use roa::App;
/// use roa::preload::*;
/// use roa::http::{StatusCode, header::COOKIE};
/// use async_std::task::spawn;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     // downstream of `cookie_parser`
///     let (addr, server) = App::new(())
///         .gate(cookie_parser)
///         .end( |mut ctx| async move {
///             assert_eq!("Hexilee", ctx.must_cookie("name")?.value());
///             Ok(())
///         })
///         .run()?;
///     spawn(server);
///     let client = reqwest::Client::new();
///     let resp = client
///         .get(&format!("http://{}", addr))
///         .header(COOKIE, "name=Hexilee")
///         .send()
///         .await?;
///     assert_eq!(StatusCode::OK, resp.status());
///
///     // miss `cookie_parser`
///     let (addr, server) = App::new(())
///         .end( |ctx| async move {
///             assert!(ctx.cookie("name").is_none());
///             Ok(())
///         })
///         .run()?;
///     spawn(server);
///     let resp = client
///         .get(&format!("http://{}", addr))
///         .header(COOKIE, "name=Hexilee")
///         .send()
///         .await?;
///     assert_eq!(StatusCode::OK, resp.status());
///     Ok(())
/// }
/// ```
pub trait CookieGetter {
    /// Must get a cookie, throw 401 UNAUTHORIZED if it not exists.
    /// ### Example
    ///
    /// ```rust
    /// use roa::cookie::cookie_parser;
    /// use roa::App;
    /// use roa::preload::*;
    /// use roa::http::{StatusCode, header::COOKIE};
    /// use async_std::task::spawn;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     // downstream of `cookie_parser`
    ///     let (addr, server) = App::new(())
    ///         .gate(cookie_parser)
    ///         .end( |mut ctx| async move {
    ///             assert_eq!("Hexilee", ctx.must_cookie("name")?.value());
    ///             Ok(())
    ///         })
    ///         .run()?;
    ///     spawn(server);
    ///     let resp = reqwest::get(&format!("http://{}", addr)).await?;
    ///     assert_eq!(StatusCode::UNAUTHORIZED, resp.status());
    ///     Ok(())
    /// }
    /// ```
    fn must_cookie(&mut self, name: &str) -> Result<Arc<Cookie<'static>>>;

    /// Try to get a cookie, return `None` if it not exists.
    /// ### Example
    ///
    /// ```rust
    /// use roa::cookie::cookie_parser;
    /// use roa::App;
    /// use roa::preload::*;
    /// use roa::http::{StatusCode, header::COOKIE};
    /// use async_std::task::spawn;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     // downstream of `cookie_parser`
    ///     let (addr, server) = App::new(())
    ///         .gate(cookie_parser)
    ///         .end( |ctx| async move {
    ///             assert!(ctx.cookie("name").is_none());
    ///             Ok(())
    ///         })
    ///         .run()?;
    ///     spawn(server);
    ///     let resp = reqwest::get(&format!("http://{}", addr)).await?;
    ///     assert_eq!(StatusCode::OK, resp.status());
    ///     Ok(())
    /// }
    /// ```
    fn cookie(&self, name: &str) -> Option<Arc<Cookie<'static>>>;
}

/// An extension to set cookie.
pub trait CookieSetter {
    /// Set a cookie in pecent encoding, should not return Err.
    /// ### Example
    ///
    /// ```rust
    /// use roa::cookie::Cookie;
    /// use roa::App;
    /// use roa::preload::*;
    /// use roa::http::{StatusCode, header::COOKIE};
    /// use async_std::task::spawn;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let (addr, server) = App::new(())
    ///         .end( |mut ctx| async move {
    ///             ctx.set_cookie(Cookie::new("name", "Hexi Lee"))?;
    ///             Ok(())
    ///         })
    ///         .run()?;
    ///     spawn(server);
    ///     let resp = reqwest::get(&format!("http://{}", addr)).await?;
    ///     assert_eq!(StatusCode::OK, resp.status());
    ///     let cookie = resp.cookies().find(|cookie| cookie.name() == "name");
    ///     assert!(cookie.is_some());
    ///     assert_eq!("Hexi%20Lee", cookie.unwrap().value());
    ///     Ok(())
    /// }
    /// ```
    fn set_cookie(&mut self, cookie: Cookie<'_>) -> Result;
}

/// A middleware to parse cookie.
pub async fn cookie_parser<S: State>(mut ctx: Context<S>, next: Next) -> Result {
    if let Some(Ok(cookies)) = ctx.header(header::COOKIE) {
        for cookie in cookies
            .split(';')
            .map(|cookie| cookie.trim())
            .map(Cookie::parse_encoded)
            .filter_map(|cookie| cookie.ok())
            .map(|cookie| cookie.into_owned())
            .collect::<Vec<Cookie<'static>>>()
            .into_iter()
        {
            let name = cookie.name().to_string();
            ctx.store_scoped(CookieScope, &name, cookie);
        }
    }
    let result = next.await;
    if let Err(ref err) = result {
        if err.status_code == StatusCode::UNAUTHORIZED
            && !ctx.resp().headers.contains_key(header::WWW_AUTHENTICATE)
        {
            ctx.resp_mut().headers.insert(
                header::WWW_AUTHENTICATE,
                err.message.parse().expect(WWW_AUTHENTICATE_BUG_HELP),
            );
        }
    }
    result
}

impl<S> CookieGetter for SyncContext<S> {
    fn must_cookie(&mut self, name: &str) -> Result<Arc<Cookie<'static>>> {
        match self.cookie(name) {
            Some(value) => Ok(value),
            None => throw!(
                StatusCode::UNAUTHORIZED,
                format!(
                    r#"Cookie name="{}""#,
                    utf8_percent_encode(name, NON_ALPHANUMERIC).to_string()
                )
            ),
        }
    }

    fn cookie(&self, name: &str) -> Option<Arc<Cookie<'static>>> {
        Some(self.load_scoped::<CookieScope, Cookie>(name)?.value())
    }
}

impl<S: State> CookieSetter for Context<S> {
    fn set_cookie(&mut self, cookie: Cookie<'_>) -> Result {
        let cookie_value = cookie.encoded().to_string();
        self.resp_mut().append(header::SET_COOKIE, cookie_value)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::cookie::{cookie_parser, Cookie};
    use crate::http::{header, StatusCode};
    use crate::preload::*;
    use crate::App;
    use async_std::task::spawn;

    #[tokio::test]
    async fn cookie() -> Result<(), Box<dyn std::error::Error>> {
        // miss cookie
        let (addr, server) = App::new(())
            .gate(cookie_parser)
            .end(move |ctx| async move {
                assert!(ctx.cookie("name").is_none());
                Ok(())
            })
            .run()?;
        spawn(server);
        reqwest::get(&format!("http://{}", addr)).await?;

        let (addr, server) = App::new(())
            .gate(cookie_parser)
            .end(move |mut ctx| async move {
                ctx.must_cookie("nick name")?;
                Ok(())
            })
            .run()?;
        spawn(server);
        let resp = reqwest::get(&format!("http://{}", addr)).await?;
        assert_eq!(StatusCode::UNAUTHORIZED, resp.status());
        assert_eq!(
            r#"Cookie name="nick%20name""#,
            resp.headers()
                .get(header::WWW_AUTHENTICATE)
                .unwrap()
                .to_str()
                .unwrap()
        );
        // string value
        let (addr, server) = App::new(())
            .gate(cookie_parser)
            .end(move |mut ctx| async move {
                assert_eq!("Hexilee", ctx.must_cookie("name")?.value());
                Ok(())
            })
            .run()?;
        spawn(server);
        let client = reqwest::Client::new();
        let resp = client
            .get(&format!("http://{}", addr))
            .header(header::COOKIE, "name=Hexilee")
            .send()
            .await?;
        assert_eq!(StatusCode::OK, resp.status());
        Ok(())
    }

    #[tokio::test]
    async fn cookie_decode() -> Result<(), Box<dyn std::error::Error>> {
        let (addr, server) = App::new(())
            .gate(cookie_parser)
            .end(move |mut ctx| async move {
                assert_eq!("bar baz", ctx.must_cookie("bar baz")?.value());
                Ok(())
            })
            .run()?;
        spawn(server);
        let client = reqwest::Client::new();
        let resp = client
            .get(&format!("http://{}", addr))
            .header(header::COOKIE, "bar%20baz=bar%20baz")
            .send()
            .await?;
        assert_eq!(StatusCode::OK, resp.status());
        Ok(())
    }

    #[tokio::test]
    async fn cookie_action() -> Result<(), Box<dyn std::error::Error>> {
        let (addr, server) = App::new(())
            .gate(cookie_parser)
            .end(move |mut ctx| async move {
                assert_eq!("bar baz", ctx.must_cookie("bar baz")?.value());
                assert_eq!("bar foo", ctx.must_cookie("foo baz")?.value());
                Ok(())
            })
            .run()?;
        spawn(server);
        let client = reqwest::Client::new();
        let resp = client
            .get(&format!("http://{}", addr))
            .header(header::COOKIE, "bar%20baz=bar%20baz; foo%20baz=bar%20foo")
            .send()
            .await?;
        assert_eq!(StatusCode::OK, resp.status());
        Ok(())
    }

    #[tokio::test]
    async fn set_cookie() -> Result<(), Box<dyn std::error::Error>> {
        let (addr, server) = App::new(())
            .end(move |mut ctx| async move {
                ctx.set_cookie(Cookie::new("bar baz", "bar baz"))?;
                ctx.set_cookie(Cookie::new("bar foo", "foo baz"))?;
                Ok(())
            })
            .run()?;
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
