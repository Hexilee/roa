use http::StatusCode;
use roa_core::{Context, Model, Status};
use std::borrow::Cow;
use std::fmt::Display;
use std::ops::Deref;
use std::str::FromStr;
use url::form_urlencoded::{parse, Parse};

#[derive(Debug)]
pub struct QueryValue<'a> {
    key: Cow<'a, str>,
    value: Cow<'a, str>,
}

pub trait Query {
    fn query(&self, key: &str) -> Result<QueryValue, Status> {
        self.try_query(key).ok_or(Status::new(
            StatusCode::BAD_REQUEST,
            format!("query `{}` is required", key),
            true,
        ))
    }
    fn try_query(&self, key: &str) -> Option<QueryValue> {
        self.queries()
            .find(|(item_key, _)| key == item_key)
            .map(|(key, value)| QueryValue { key, value })
    }
    fn queries(&self) -> Parse;
}

impl<M: Model> Query for Context<M> {
    fn queries(&self) -> Parse {
        let query_string = self.request.uri.query().unwrap_or("");
        parse(query_string.as_bytes())
    }
}

impl Deref for QueryValue<'_> {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl AsRef<str> for QueryValue<'_> {
    fn as_ref(&self) -> &str {
        &self
    }
}

impl QueryValue<'_> {
    pub fn parse<T>(&self) -> Result<T, Status>
    where
        T: FromStr,
        T::Err: Display,
    {
        self.as_ref().parse().map_err(|err| {
            Status::new(
                StatusCode::BAD_REQUEST,
                format!("{}\nquery `{}` type mismatch", err, self.key),
                true,
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::Query;
    use roa_core::{Context, Ctx, Group, Request};

    #[test]
    fn query() -> Result<(), Box<dyn std::error::Error>> {
        // miss key
        let mut ctx: Context<()> = Ctx::fake(Request::new()).into();
        assert!(ctx.query("name").is_err());
        assert_eq!(
            "query `name` is required",
            ctx.query("name").unwrap_err().message
        );

        // string value
        let mut req = Request::new();
        req.uri = "/?name=Hexilee".parse()?;
        ctx = Ctx::fake(req).into();
        assert_eq!("Hexilee", ctx.query("name")?.as_ref());
        Ok(())
    }

    #[test]
    fn query_parse() -> Result<(), Box<dyn std::error::Error>> {
        // invalid int value
        let mut req = Request::new();
        req.uri = "/?age=Hexilee".parse()?;
        let mut ctx: Context<()> = Ctx::fake(req).into();
        let result = ctx.query("age")?.parse::<u64>();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .message
            .contains("query `age` type mismatch"));

        let mut req = Request::new();
        req.uri = "/?age=120".parse()?;
        ctx = Ctx::fake(req).into();
        let age: i32 = ctx.query("age")?.parse()?;
        assert_eq!(120, age);
        Ok(())
    }

    #[tokio::test]
    async fn query_action() -> Result<(), Box<dyn std::error::Error>> {
        let mut request = Request::new();
        request.uri = "/?name=Hexilee&lang=rust".parse()?;
        Group::<()>::new()
            .handle_fn(move |ctx, _next| {
                async move {
                    assert_eq!("Hexilee", ctx.query("name")?.as_ref());
                    assert_eq!("rust", ctx.query("lang")?.as_ref());
                    Ok(())
                }
            })
            .app(())
            .serve(request, "127.0.0.1:8000".parse()?)
            .await?;
        Ok(())
    }
}
