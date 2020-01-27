use futures::AsyncRead as Read;
use std::pin::Pin;

pub type BodyRef<'a> = &'a mut (dyn Read + Send + Unpin + 'static);

pub struct Request {
    req: http_service::Request,
}

impl Request {
    pub fn new(req: http_service::Request) -> Self {
        Self { req }
    }

    pub fn body(&mut self) -> Pin<BodyRef> {
        Pin::new(self.req.body_mut())
    }
}

#[cfg(test)]
mod tests {
    use crate::App;
    use futures::AsyncReadExt;
    use http_service::{Body, Request};
    use std::convert::Infallible;

    #[async_std::test]
    async fn body_read() -> Result<(), Infallible> {
        let _resp = App::builder()
            .handle_fn(|mut ctx, _next| {
                async move {
                    let mut data = String::new();
                    ctx.request.body().read_to_string(&mut data).await?;
                    assert_eq!("Hello, World!", data);
                    Ok(())
                }
            })
            .model(())
            .serve(Request::new(Body::from(b"Hello, World!".to_vec())))
            .await;
        Ok(())
    }
}
