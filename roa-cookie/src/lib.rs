use async_trait::async_trait;
pub use cookie::Cookie;
use http::header;
use roa_core::{Context, Model, Status};

#[async_trait]
pub trait Cookier {
    async fn cookies<'a>(&'a self) -> Result<Box<dyn Iterator<Item = Cookie<'a>> + 'a>, Status>;
    async fn cookie(&self, key: &str) -> Result<Option<Cookie<'static>>, Status> {
        Ok(self.cookies()?.find(|cookie| cookie.name() == key))
    }
    async fn set_cookie(&mut self, cookie: &Cookie) -> Result<(), Status>;
}

#[async_trait]
impl<M: Model> Cookier for Context<M> {
    async fn cookies<'a>(&'a self) -> Result<Box<dyn Iterator<Item = Cookie<'a>> + 'a>, Status> {
        Ok(match self.req().await.headers.get(header::COOKIE) {
            None => Box::new(vec![].into_iter()),
            Some(cookies) => Box::new(
                cookies
                    .to_str()?
                    .split(';')
                    .map(Cookie::parse_encoded)
                    .filter_map(|cookie| cookie.ok()),
            ),
        })
    }

    async fn set_cookie(&mut self, cookie: &Cookie) -> Result<(), Status> {
        let cookie_value = cookie.encoded().to_string().parse()?;

        if self.response.headers.contains_key(header::SET_COOKIE) {
            self.response
                .headers
                .append(header::SET_COOKIE, cookie_value);
        } else {
            self.response
                .headers
                .insert(header::SET_COOKIE, cookie_value);
        }
        Ok(())
    }
}
