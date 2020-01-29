use roa_core::{Context, Model, Next, Status};
use url::form_urlencoded::parse;

pub trait QueryStorage {
    fn insert_pair(&mut self, key: &str, value: &str);
}

pub async fn query_parser<M: Model>(mut ctx: Context<M>, next: Next) -> Result<(), Status>
where
    M::State: QueryStorage,
{
    if let Some(query) = ctx.request.uri.query() {
        for (key, value) in parse(query.to_string().as_bytes()) {
            ctx.insert_pair(&key, &value)
        }
    }
    next().await
}

#[cfg(test)]
mod tests {
    use crate::query_parser::{query_parser, QueryStorage};
    use roa_core::{Group, Model, Request};
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
