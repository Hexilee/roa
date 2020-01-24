use futures::io::AsyncRead;
use futures::stream::TryStreamExt;
use std::io;
use std::pin::Pin;

pub type Body = Pin<Box<dyn AsyncRead + Sync + Send + 'static>>;
pub type BodyRef<'a> = &'a mut (dyn AsyncRead + Sync + Send + 'static);

pub struct Request {
    req: http::Request<Body>,
}

impl Request {
    pub fn new(req: http::Request<hyper::Body>) -> Self {
        let (parts, body) = req.into_parts();
        Self {
            req: http::Request::from_parts(parts, Box::pin(Self::body_into_async_read(body))),
        }
    }

    pub fn body(&mut self) -> Pin<BodyRef> {
        self.req.body_mut().as_mut()
    }

    fn body_into_async_read(body: hyper::Body) -> impl AsyncRead {
        body.map_err(|err: hyper::Error| {
            let kind = if err.is_incomplete_message() {
                io::ErrorKind::UnexpectedEof
            } else {
                io::ErrorKind::BrokenPipe
            };
            io::Error::new(kind, err)
        })
        .into_async_read()
    }
}

#[cfg(test)]
mod tests {
    use crate::App;
    use futures::AsyncReadExt;
    use hyper::{Body, Request};
    use std::convert::Infallible;

    #[tokio::test]
    async fn test_body_stream() -> Result<(), Infallible> {
        let _resp = App::<()>::new()
            .gate(|ctx, _next| {
                Box::pin(async move {
                    let mut data = String::new();
                    ctx.request.body().read_to_string(&mut data).await.unwrap();
                    assert_eq!("Hello, World!", data);
                    Ok(())
                })
            })
            .leak()
            .serve(Request::new(Body::from("Hello, World!")))
            .await?;
        Ok(())
    }
}
