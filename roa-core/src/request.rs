use bytes::Bytes;
use futures::stream::TryStreamExt;
use futures::{AsyncRead, Stream};
use http::{HeaderMap, HeaderValue, Method, Uri, Version};
use hyper::Body;
use std::io;
use std::mem::swap;

/// Http request type of roa.
#[derive(Default)]
pub struct Request {
    /// The request's method
    pub method: Method,

    /// The request's URI
    pub uri: Uri,

    /// The request's version
    pub version: Version,

    /// The request's headers
    pub headers: HeaderMap<HeaderValue>,

    body: Body,
}

impl Request {
    /// Get raw hyper body.
    #[inline]
    pub fn raw_body(&mut self) -> Body {
        std::mem::take(&mut self.body)
    }
    /// Get body as Stream.
    /// This method will consume inner body.
    #[inline]
    pub fn stream(
        &mut self,
    ) -> impl Stream<Item = io::Result<Bytes>> + Sync + Send + Unpin + 'static {
        self.raw_body()
            .map_err(|err| io::Error::new(io::ErrorKind::Other, err))
    }

    /// Get body as AsyncRead.
    /// This method will consume inner body.
    #[inline]
    pub fn reader(&mut self) -> impl AsyncRead + Sync + Send + Unpin + 'static {
        self.stream().into_async_read()
    }
}

impl Request {
    pub(crate) fn reload(&mut self, req: &mut http::Request<Body>) {
        swap(&mut self.method, req.method_mut());
        swap(&mut self.uri, req.uri_mut());
        swap(&mut self.headers, req.headers_mut());
        swap(&mut self.version, req.version_mut());
        swap(&mut self.body, req.body_mut());
    }
}

#[cfg(all(test, feature = "runtime"))]
mod tests {
    use crate::App;
    use futures::AsyncReadExt;
    use http::StatusCode;
    use hyper::service::Service;
    use hyper::{Body, Request};

    #[async_std::test]
    async fn body_read() -> Result<(), Box<dyn std::error::Error>> {
        let mut app = App::new(());
        app.gate_fn(|mut ctx, _next| async move {
            let mut data = String::new();
            ctx.req_mut().reader().read_to_string(&mut data).await?;
            assert_eq!("Hello, World!", data);
            Ok(())
        });
        let mut service = app.http_service();
        let resp = service
            .call(Request::new(Body::from("Hello, World!")))
            .await?;
        assert_eq!(StatusCode::OK, resp.status());
        Ok(())
    }
}
