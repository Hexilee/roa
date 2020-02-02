use async_trait::async_trait;
pub use cookie::Cookie;
use http::header;
use roa_core::{Context, Model, Status};

#[async_trait]
pub trait Cookier {
    async fn cookies(&self) -> Result<Vec<Cookie<'static>>, Status>;
    async fn cookie(&self, key: &str) -> Result<Option<Cookie<'static>>, Status> {
        Ok(self
            .cookies()
            .await?
            .into_iter()
            .find(|cookie| cookie.name() == key))
    }
    async fn set_cookie(&self, cookie: Cookie<'_>) -> Result<(), Status>;
}

#[async_trait]
impl<M: Model> Cookier for Context<M> {
    async fn cookies(&self) -> Result<Vec<Cookie<'static>>, Status> {
        Ok(match self.header(&header::COOKIE).await {
            None => vec![],
            Some(cookies) => cookies
                .to_str()?
                .split(';')
                .map(Cookie::parse_encoded)
                .filter_map(|cookie| cookie.ok())
                .map(|cookie| cookie.into_owned())
                .collect(),
        })
    }

    async fn set_cookie(&self, cookie: Cookie<'_>) -> Result<(), Status> {
        let cookie_value = cookie.encoded().to_string().parse()?;
        if self.resp().await.headers.contains_key(header::SET_COOKIE) {
            self.resp()
                .await
                .headers
                .append(header::SET_COOKIE, cookie_value);
        } else {
            self.resp()
                .await
                .headers
                .insert(header::SET_COOKIE, cookie_value);
        }
        Ok(())
    }
}
