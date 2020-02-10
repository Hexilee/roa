use crate::core::{Context, Error, Model, Next, Result};
use crate::header::FriendlyHeaders;
use async_trait::async_trait;
pub use cookie::Cookie;
use http::{header, StatusCode};
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};

const WWW_AUTHENTICATE_BUG_HELP: &str = "
Invalid WWW_AUTHENTICATE value, this is a bug of roa::cookie.
Please report it to https://github.com/Hexilee/roa.
";

struct CookieSymbol;

#[async_trait]
pub trait Cookier {
    async fn cookie(&self, name: &str) -> Result<String>;
    async fn try_cookie(&self, name: &str) -> Option<String>;
    async fn set_cookie(&self, cookie: Cookie<'_>) -> Result;
}

pub async fn cookie_parser<M: Model>(ctx: Context<M>, next: Next) -> Result {
    if let Some(Ok(cookies)) = ctx.header(header::COOKIE).await {
        for cookie in cookies
            .split(';')
            .map(|cookie| cookie.trim())
            .map(Cookie::parse_encoded)
            .filter_map(|cookie| cookie.ok())
        {
            ctx.store::<CookieSymbol>(cookie.name(), cookie.value().to_string())
                .await;
        }
    }
    next().await
}

#[async_trait]
impl<M: Model> Cookier for Context<M> {
    async fn cookie(&self, name: &str) -> Result<String> {
        match self.try_cookie(name).await {
            Some(value) => Ok(value),
            None => {
                let www_authenticate = format!(
                    r#"Cookie name="{}""#,
                    utf8_percent_encode(name, NON_ALPHANUMERIC).to_string()
                );
                self.resp_mut().await.headers.insert(
                    header::WWW_AUTHENTICATE,
                    www_authenticate.parse().expect(WWW_AUTHENTICATE_BUG_HELP),
                );
                Err(Error::new(StatusCode::UNAUTHORIZED, "", false))
            }
        }
    }
    async fn try_cookie(&self, name: &str) -> Option<String> {
        self.load::<CookieSymbol>(name)
            .await
            .map(|var| var.into_value())
    }
    async fn set_cookie(&self, cookie: Cookie<'_>) -> Result {
        let cookie_value = cookie.encoded().to_string();
        self.resp_mut()
            .await
            .append(header::SET_COOKIE, cookie_value)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{cookie_parser, Cookie, Cookier};
    use crate::core::App;
    use async_std::task::spawn;
    use http::{header, StatusCode};

    #[tokio::test]
    async fn cookie() -> Result<(), Box<dyn std::error::Error>> {
        // miss cookie
        let (addr, server) = App::new(())
            .gate(cookie_parser)
            .gate(move |ctx, _next| async move {
                assert!(ctx.try_cookie("name").await.is_none());
                Ok(())
            })
            .run_local()?;
        spawn(server);
        reqwest::get(&format!("http://{}", addr)).await?;

        let (addr, server) = App::new(())
            .gate(cookie_parser)
            .gate(move |ctx, _next| async move {
                ctx.cookie("nick name").await?;
                Ok(())
            })
            .run_local()?;
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
            .gate(move |ctx, _next| async move {
                assert_eq!("Hexilee", ctx.cookie("name").await?);
                Ok(())
            })
            .run_local()?;
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
            .gate(move |ctx, _next| async move {
                assert_eq!("bar baz", ctx.cookie("bar baz").await?);
                Ok(())
            })
            .run_local()?;
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
            .gate(move |ctx, _next| async move {
                assert_eq!("bar baz", ctx.cookie("bar baz").await?);
                assert_eq!("bar foo", ctx.cookie("foo baz").await?);
                Ok(())
            })
            .run_local()?;
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
            .gate(move |ctx, _next| async move {
                ctx.set_cookie(Cookie::new("bar baz", "bar baz")).await?;
                ctx.set_cookie(Cookie::new("bar foo", "foo baz")).await?;
                Ok(())
            })
            .run_local()?;
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
