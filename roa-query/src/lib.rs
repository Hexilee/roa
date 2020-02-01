use async_trait::async_trait;
use http::StatusCode;
use roa_core::{Context, Model, Status};
use std::fmt::Display;
use std::ops::Deref;
use std::str::FromStr;
use url::form_urlencoded::parse;

#[derive(Debug)]
pub struct Variable {
    name: String,
    value: String,
}

#[async_trait]
pub trait Query {
    async fn query(&self, name: &str) -> Result<Variable, Status>;
    async fn try_query(&self, name: &str) -> Option<Variable>;
    async fn queries(&self) -> Vec<(String, String)>;
}

#[async_trait]
impl<M: Model> Query for Context<M> {
    async fn query(&self, name: &str) -> Result<Variable, Status> {
        self.try_query(name).await.ok_or(Status::new(
            StatusCode::BAD_REQUEST,
            format!("query `{}` is required", name),
            true,
        ))
    }
    async fn try_query(&self, name: &str) -> Option<Variable> {
        self.queries()
            .await
            .into_iter()
            .find(|(item_name, _)| name == item_name)
            .map(|(name, value)| Variable { name, value })
    }

    async fn queries(&self) -> Vec<(String, String)> {
        let uri = self.uri().await;
        let query_string = uri.query().unwrap_or("");
        parse(query_string.as_bytes())
            .map(|(key, value)| (key.into_owned(), value.into_owned()))
            .collect()
    }
}

impl Deref for Variable {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl AsRef<str> for Variable {
    fn as_ref(&self) -> &str {
        &self
    }
}

impl Variable {
    pub fn parse<T>(&self) -> Result<T, Status>
    where
        T: FromStr,
        T::Err: Display,
    {
        self.as_ref().parse().map_err(|err| {
            Status::new(
                StatusCode::BAD_REQUEST,
                format!("{}\nuri variable `{}` type mismatch", err, self.name),
                true,
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::Query;
    use roa_core::{App, Context, Request};

    #[tokio::test]
    async fn query() -> Result<(), Box<dyn std::error::Error>> {
        // miss key
        let mut ctx: Context<()> = Context::fake(Request::new());
        assert!(ctx.query("name").await.is_err());
        assert_eq!(
            "query `name` is required",
            ctx.query("name").await.unwrap_err().message
        );

        // string value
        let mut req = Request::new();
        req.uri = "/?name=Hexilee".parse()?;
        ctx = Context::fake(req);
        assert_eq!("Hexilee", ctx.query("name").await?.as_ref());
        Ok(())
    }

    #[tokio::test]
    async fn query_parse() -> Result<(), Box<dyn std::error::Error>> {
        // invalid int value
        let mut req = Request::new();
        req.uri = "/?age=Hexilee".parse()?;
        let mut ctx: Context<()> = Context::fake(req);
        let result = ctx.query("age").await?.parse::<u64>();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .message
            .contains("uri variable `age` type mismatch"));

        let mut req = Request::new();
        req.uri = "/?age=120".parse()?;
        ctx = Context::fake(req);
        let age: i32 = ctx.query("age").await?.parse()?;
        assert_eq!(120, age);
        Ok(())
    }

    #[tokio::test]
    async fn query_action() -> Result<(), Box<dyn std::error::Error>> {
        let mut request = Request::new();
        request.uri = "/?name=Hexilee&lang=rust".parse()?;
        App::new(())
            .join(move |ctx, _next| {
                async move {
                    assert_eq!("Hexilee", ctx.query("name").await?.as_ref());
                    assert_eq!("rust", ctx.query("lang").await?.as_ref());
                    Ok(())
                }
            })
            .serve(request, "127.0.0.1:8000".parse()?)
            .await?;
        Ok(())
    }
}
