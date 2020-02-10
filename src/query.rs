use crate::core::{Context, Error, Model, Next, Result, Variable};
use async_trait::async_trait;
use http::StatusCode;
use url::form_urlencoded::parse;

struct QuerySymbol;

#[async_trait]
pub trait Query {
    async fn query<'a>(&self, name: &'a str) -> Result<Variable<'a>>;
    async fn try_query<'a>(&self, name: &'a str) -> Option<Variable<'a>>;
}

pub async fn query_parser<M: Model>(ctx: Context<M>, next: Next) -> Result {
    let uri = ctx.uri().await;
    let query_string = uri.query().unwrap_or("");
    for (key, value) in parse(query_string.as_bytes()) {
        ctx.store::<QuerySymbol>(&key, value.to_string()).await;
    }
    next().await
}

#[async_trait]
impl<M: Model> Query for Context<M> {
    async fn query<'a>(&self, name: &'a str) -> Result<Variable<'a>> {
        self.try_query(name).await.ok_or_else(|| {
            Error::new(
                StatusCode::BAD_REQUEST,
                format!("query `{}` is required", name),
                true,
            )
        })
    }
    async fn try_query<'a>(&self, name: &'a str) -> Option<Variable<'a>> {
        self.load::<QuerySymbol>(name).await
    }
}

#[cfg(test)]
mod tests {
    use super::{query_parser, Query};
    use crate::core::App;
    use async_std::task::spawn;
    use http::StatusCode;

    #[tokio::test]
    async fn query() -> Result<(), Box<dyn std::error::Error>> {
        // miss key
        let (addr, server) = App::new(())
            .gate(query_parser)
            .gate(move |ctx, _next| async move {
                assert!(ctx.try_query("name").await.is_none());
                Ok(())
            })
            .run_local()?;
        spawn(server);
        reqwest::get(&format!("http://{}/", addr)).await?;

        // string value
        let (addr, server) = App::new(())
            .gate(query_parser)
            .gate(move |ctx, _next| async move {
                assert_eq!("Hexilee", ctx.query("name").await?.as_ref());
                Ok(())
            })
            .run_local()?;
        spawn(server);
        let resp = reqwest::get(&format!("http://{}?name=Hexilee", addr)).await?;
        assert_eq!(StatusCode::OK, resp.status());
        Ok(())
    }

    #[tokio::test]
    async fn query_parse() -> Result<(), Box<dyn std::error::Error>> {
        // invalid int value
        let (addr, server) = App::new(())
            .gate(query_parser)
            .gate(move |ctx, _next| async move {
                assert!(ctx.query("age").await?.parse::<u64>().is_err());
                Ok(())
            })
            .run_local()?;
        spawn(server);
        let resp = reqwest::get(&format!("http://{}?age=Hexilee", addr)).await?;
        assert_eq!(StatusCode::OK, resp.status());

        let (addr, server) = App::new(())
            .gate(query_parser)
            .gate(move |ctx, _next| async move {
                assert_eq!(120, ctx.query("age").await?.parse::<u64>()?);
                Ok(())
            })
            .run_local()?;
        spawn(server);
        let resp = reqwest::get(&format!("http://{}?age=120", addr)).await?;
        assert_eq!(StatusCode::OK, resp.status());
        Ok(())
    }

    #[tokio::test]
    async fn query_action() -> Result<(), Box<dyn std::error::Error>> {
        let (addr, server) = App::new(())
            .gate(query_parser)
            .gate(move |ctx, _next| async move {
                assert_eq!("Hexilee", ctx.query("name").await?.as_ref());
                assert_eq!("rust", ctx.query("lang").await?.as_ref());
                Ok(())
            })
            .run_local()?;
        spawn(server);
        let resp = reqwest::get(&format!("http://{}?name=Hexilee&lang=rust", addr)).await?;
        assert_eq!(StatusCode::OK, resp.status());
        Ok(())
    }
}
