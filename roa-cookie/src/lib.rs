pub use cookie::Cookie;
use http::header;
use roa_core::{Context, Model, Status};

pub trait Cookier {
    fn cookies<'a>(&'a self) -> Result<Box<dyn Iterator<Item = Cookie<'a>> + 'a>, Status>;
    fn cookie(&self, key: &str) -> Result<Option<Cookie>, Status> {
        Ok(self.cookies()?.find(|cookie| cookie.name() == key))
    }
    fn set_cookie(&mut self, cookie: &Cookie) -> Result<(), Status>;
}

impl<M: Model> Cookier for Context<M> {
    fn cookies<'a>(&'a self) -> Result<Box<dyn Iterator<Item = Cookie<'a>> + 'a>, Status> {
        Ok(match self.request.headers.get(header::COOKIE) {
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

    fn set_cookie(&mut self, cookie: &Cookie) -> Result<(), Status> {
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
