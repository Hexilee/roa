use crate::{Context, Model, Next, Status};
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
    async fn cookie(&self, name: &str) -> Result<String, Status>;
    async fn try_cookie(&self, name: &str) -> Option<String>;
    async fn set_cookie(&self, cookie: Cookie<'_>) -> Result<(), Status>;
}

pub async fn cookie_parser<M: Model>(ctx: Context<M>, next: Next) -> Result<(), Status> {
    if let Some(Ok(cookies)) = ctx.header(&header::COOKIE).await {
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
    async fn cookie(&self, name: &str) -> Result<String, Status> {
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
                Err(Status::new(StatusCode::UNAUTHORIZED, "", false))
            }
        }
    }
    async fn try_cookie(&self, name: &str) -> Option<String> {
        self.load::<CookieSymbol>(name)
            .await
            .map(|var| var.into_value())
    }
    async fn set_cookie(&self, cookie: Cookie<'_>) -> Result<(), Status> {
        let cookie_value = cookie.encoded().to_string().parse()?;
        self.resp_mut()
            .await
            .headers
            .append(header::SET_COOKIE, cookie_value);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{cookie_parser, Cookie, Cookier};
    use crate::{App, Request};
    use http::{header, StatusCode};

    #[tokio::test]
    async fn cookie() -> Result<(), Box<dyn std::error::Error>> {
        // miss cookie
        let mut req = Request::new();
        App::new(())
            .gate(cookie_parser)
            .gate(move |ctx, _next| async move {
                assert!(ctx.try_cookie("name").await.is_none());
                Ok(())
            })
            .serve(req, "127.0.0.1:8000".parse()?)
            .await?;

        req = Request::new();
        let resp = App::new(())
            .gate(cookie_parser)
            .gate(move |ctx, _next| async move {
                ctx.cookie("nick name").await?;
                Ok(())
            })
            .serve(req, "127.0.0.1:8000".parse()?)
            .await?;

        assert_eq!(StatusCode::UNAUTHORIZED, resp.status);
        assert_eq!(
            r#"Cookie name="nick%20name""#,
            resp.headers
                .get(header::WWW_AUTHENTICATE)
                .unwrap()
                .to_str()
                .unwrap()
        );
        // string value
        req = Request::new();
        req.headers.insert(header::COOKIE, "name=Hexilee".parse()?);
        let resp = App::new(())
            .gate(cookie_parser)
            .gate(move |ctx, _next| async move {
                assert_eq!("Hexilee", ctx.cookie("name").await?);
                Ok(())
            })
            .serve(req, "127.0.0.1:8000".parse()?)
            .await?;
        assert_eq!(StatusCode::OK, resp.status);
        Ok(())
    }

    #[tokio::test]
    async fn cookie_decode() -> Result<(), Box<dyn std::error::Error>> {
        // invalid int value
        let mut req = Request::new();
        req.headers
            .insert(header::COOKIE, "bar%20baz=bar%20baz".parse()?);
        let resp = App::new(())
            .gate(cookie_parser)
            .gate(move |ctx, _next| async move {
                assert_eq!("bar baz", ctx.cookie("bar baz").await?);
                Ok(())
            })
            .serve(req, "127.0.0.1:8000".parse()?)
            .await?;
        assert_eq!(StatusCode::OK, resp.status);
        Ok(())
    }

    #[tokio::test]
    async fn cookie_action() -> Result<(), Box<dyn std::error::Error>> {
        let mut req = Request::new();
        req.headers.insert(
            header::COOKIE,
            "bar%20baz=bar%20baz; foo%20baz=bar%20foo".parse()?,
        );
        let resp = App::new(())
            .gate(cookie_parser)
            .gate(move |ctx, _next| async move {
                assert_eq!("bar baz", ctx.cookie("bar baz").await?);
                assert_eq!("bar foo", ctx.cookie("foo baz").await?);
                Ok(())
            })
            .serve(req, "127.0.0.1:8000".parse()?)
            .await?;
        assert_eq!(StatusCode::OK, resp.status);
        Ok(())
    }

    #[tokio::test]
    async fn set_cookie() -> Result<(), Box<dyn std::error::Error>> {
        let resp = App::new(())
            .gate(move |ctx, _next| async move {
                ctx.set_cookie(Cookie::new("bar baz", "bar baz")).await?;
                ctx.set_cookie(Cookie::new("bar foo", "foo baz")).await?;
                Ok(())
            })
            .serve(Request::new(), "127.0.0.1:8000".parse()?)
            .await?;
        assert_eq!(StatusCode::OK, resp.status);
        let set_cookies = resp.headers.get_all(header::SET_COOKIE);
        let cookies: Vec<Cookie> = set_cookies
            .iter()
            .map(|value| value.to_str().unwrap())
            .map(Cookie::parse_encoded)
            .filter_map(|cookie| cookie.ok())
            .collect();
        assert_eq!(2, cookies.len());
        assert_eq!(("bar baz", "bar baz"), cookies[0].name_value());
        assert_eq!(("bar foo", "foo baz"), cookies[1].name_value());
        Ok(())
    }
}
