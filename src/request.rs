use crate::Body;
use futures::{AsyncRead as Read, TryStreamExt};
use std::io;
use std::ops::{Deref, DerefMut};
use std::pin::Pin;

pub type BodyRef<'a> = &'a mut (dyn Read + Send + Unpin + 'static);

pub struct Request(http::Request<Body>);

impl Request {
    pub fn new() -> Self {
        Self(http::Request::new(Body::new()))
    }
}

impl Deref for Request {
    type Target = Body;
    fn deref(&self) -> &Self::Target {
        self.0.body()
    }
}

impl DerefMut for Request {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.body_mut()
    }
}

impl From<http::Request<Body>> for Request {
    fn from(req: http::Request<Body>) -> Self {
        Self(req)
    }
}

impl From<http::Request<hyper::Body>> for Request {
    fn from(req: http::Request<hyper::Body>) -> Self {
        let (parts, body) = req.into_parts();
        let mut new_req = Self::new();
        new_req.write(
            body.map_err(|err| io::Error::new(io::ErrorKind::Other, err))
                .into_async_read(),
        );
        new_req
    }
}

#[cfg(test)]
mod tests {
    use crate::{App, HttpService, Request};
    use futures::AsyncReadExt;
    use std::convert::Infallible;

    #[async_std::test]
    async fn body_read() -> Result<(), Box<dyn std::error::Error>> {
        let app = App::builder()
            .handle_fn(|mut ctx, _next| {
                async move {
                    let mut data = String::new();
                    ctx.request.read_to_string(&mut data).await?;
                    assert_eq!("Hello, World!", data);
                    Ok(())
                }
            })
            .model(());
        let _resp = HttpService::new(app, "127.0.0.1:8080".parse()?)
            .serve(Request::new())
            .await?;
        Ok(())
    }
}
