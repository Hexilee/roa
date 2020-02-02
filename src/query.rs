use crate::{Context, Model, Next, Status, Variable};
use async_trait::async_trait;
use http::StatusCode;
use url::form_urlencoded::parse;

struct QuerySymbol;

#[async_trait]
pub trait Query {
    async fn query<'a>(&self, name: &'a str) -> Result<Variable<'a>, Status>;
    async fn try_query<'a>(&self, name: &'a str) -> Option<Variable<'a>>;
}

pub async fn query_parser<M: Model>(ctx: Context<M>, next: Next) -> Result<(), Status> {
    let uri = ctx.uri().await;
    let query_string = uri.query().unwrap_or("");
    for (key, value) in parse(query_string.as_bytes()) {
        ctx.store::<QuerySymbol>(&key, value.to_string()).await;
    }
    next().await
}

#[async_trait]
impl<M: Model> Query for Context<M> {
    async fn query<'a>(&self, name: &'a str) -> Result<Variable<'a>, Status> {
        self.try_query(name).await.ok_or(Status::new(
            StatusCode::BAD_REQUEST,
            format!("query `{}` is required", name),
            true,
        ))
    }
    async fn try_query<'a>(&self, name: &'a str) -> Option<Variable<'a>> {
        self.load::<QuerySymbol>(name).await
    }
}

#[cfg(test)]
mod tests {
    use super::{query_parser, Query};
    use crate::{App, Request};
    use http::StatusCode;

    #[tokio::test]
    async fn query() -> Result<(), Box<dyn std::error::Error>> {
        // miss key
        let mut req = Request::new();
        req.uri = "/".parse()?;
        App::new(())
            .join(query_parser)
            .join(move |ctx, _next| async move {
                assert!(ctx.try_query("name").await.is_none());
                Ok(())
            })
            .serve(req, "127.0.0.1:8000".parse()?)
            .await?;

        // string value
        req = Request::new();
        req.uri = "/?name=Hexilee".parse()?;
        let resp = App::new(())
            .join(query_parser)
            .join(move |ctx, _next| async move {
                assert_eq!("Hexilee", ctx.query("name").await?.as_ref());
                Ok(())
            })
            .serve(req, "127.0.0.1:8000".parse()?)
            .await?;
        assert_eq!(StatusCode::OK, resp.status);
        Ok(())
    }

    #[tokio::test]
    async fn query_parse() -> Result<(), Box<dyn std::error::Error>> {
        // invalid int value
        let mut req = Request::new();
        req.uri = "/?age=Hexilee".parse()?;
        let resp = App::new(())
            .join(query_parser)
            .join(move |ctx, _next| async move {
                assert!(ctx.query("age").await?.parse::<u64>().is_err());
                Ok(())
            })
            .serve(req, "127.0.0.1:8000".parse()?)
            .await?;
        assert_eq!(StatusCode::OK, resp.status);

        req = Request::new();
        req.uri = "/?age=120".parse()?;
        let resp = App::new(())
            .join(query_parser)
            .join(move |ctx, _next| async move {
                assert_eq!(120, ctx.query("age").await?.parse::<u64>()?);
                Ok(())
            })
            .serve(req, "127.0.0.1:8000".parse()?)
            .await?;
        assert_eq!(StatusCode::OK, resp.status);
        Ok(())
    }

    #[tokio::test]
    async fn query_action() -> Result<(), Box<dyn std::error::Error>> {
        let mut request = Request::new();
        request.uri = "/?name=Hexilee&lang=rust".parse()?;
        let resp = App::new(())
            .join(query_parser)
            .join(move |ctx, _next| async move {
                assert_eq!("Hexilee", ctx.query("name").await?.as_ref());
                assert_eq!("rust", ctx.query("lang").await?.as_ref());
                Ok(())
            })
            .serve(request, "127.0.0.1:8000".parse()?)
            .await?;
        assert_eq!(StatusCode::OK, resp.status);
        Ok(())
    }
}
