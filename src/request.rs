use futures::AsyncRead as Read;
use std::pin::Pin;

pub type BodyRef<'a> = &'a mut (dyn Read + Send + 'static);

pub struct Request {
    req: http_service::Request,
}

impl Request {
    pub fn new(req: http_service::Request) -> Self {
        Self { req }
    }

    pub fn body(&mut self) -> Pin<BodyRef> {
        unsafe { Pin::new_unchecked(self.req.body_mut()) }
    }
}

#[cfg(test)]
mod tests {
    use crate::Server;
    use futures::AsyncReadExt;
    use http_service::{Body, Request};
    use std::convert::Infallible;

    #[async_std::test]
    async fn test_body_stream() -> Result<(), Infallible> {
        let _resp = Server::<()>::new()
            .gate(|ctx, _next| {
                Box::pin(async move {
                    let mut data = String::new();
                    ctx.request.body().read_to_string(&mut data).await?;
                    assert_eq!("Hello, World!", data);
                    Ok(())
                })
            })
            .into_service()
            .serve(Request::new(Body::from(b"Hello, World!".to_vec())))
            .await;
        Ok(())
    }
}
