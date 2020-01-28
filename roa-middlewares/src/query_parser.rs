use roa::{Context, Model, Next, Status, StatusCode};
use url::Url;

pub trait QueryStorage {
    fn insert_pair(&mut self, key: &str, value: &str);
}

pub async fn query_parser<M: Model>(mut ctx: Context<M>, next: Next) -> Result<(), Status>
where
    M::State: QueryStorage,
{
    let url = Url::parse(&ctx.request.uri.to_string()).map_err(|err| {
        Status::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("this is a bug of crate http or url: {}", err),
            false,
        )
    })?;
    for (key, value) in url.query_pairs() {
        ctx.insert_pair(&key, &value)
    }
    next().await
}

#[cfg(test)]
mod tests {
    use crate::query_parser::{query_parser, QueryStorage};
    use roa::{Group, Model, Request};
    use std::collections::HashMap;

    struct AppModel {}
    struct AppState {
        query: HashMap<String, String>,
    }

    impl Model for AppModel {
        type State = AppState;
        fn new_state(&self) -> Self::State {
            AppState {
                query: HashMap::new(),
            }
        }
    }

    impl QueryStorage for AppState {
        fn insert_pair(&mut self, key: &str, value: &str) {
            self.query.insert(key.to_owned(), value.to_owned());
        }
    }

    #[tokio::test]
    async fn query_parse() -> Result<(), Box<dyn std::error::Error>> {
        let mut request = Request::new();
        request.uri = "http://github.com?name=Hexilee&lang=rust".parse()?;
        Group::<AppModel>::new()
            .handle_fn(query_parser)
            .handle_fn(move |ctx, _next| {
                async move {
                    assert_eq!("Hexilee", &ctx.query["name"]);
                    assert_eq!("rust", &ctx.query["lang"]);
                    Ok(())
                }
            })
            .app(AppModel {})
            .serve(request, "127.0.0.1:8000".parse()?)
            .await?;
        Ok(())
    }
}
