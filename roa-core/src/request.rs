use bytes::Bytes;
use futures::stream::TryStreamExt;
use futures::{AsyncRead, Stream};
use http::{HeaderMap, HeaderValue, Method, Uri, Version};
use hyper::Body;
use std::io;

/// Http request type of roa.
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

impl From<http::Request<Body>> for Request {
    #[inline]
    fn from(req: http::Request<Body>) -> Self {
        let (parts, body) = req.into_parts();
        Self {
            method: parts.method,
            uri: parts.uri,
            version: parts.version,
            headers: parts.headers,
            body,
        }
    }
}

impl Default for Request {
    #[inline]
    fn default() -> Self {
        http::Request::new(Body::empty()).into()
    }
}

#[cfg(all(test, feature = "runtime"))]
mod tests {
    use crate::{App, Context, Request, Status};
    use futures::AsyncReadExt;
    use http::StatusCode;
    use hyper::Body;

    async fn test(ctx: &mut Context<()>) -> Result<(), Status> {
        let mut data = String::new();
        ctx.req.reader().read_to_string(&mut data).await?;
        assert_eq!("Hello, World!", data);
        Ok(())
    }

    #[async_std::test]
    async fn body_read() -> Result<(), Box<dyn std::error::Error>> {
        let app = App::new(()).end(test);
        let service = app.http_service();
        let req = Request::from(http::Request::new(Body::from("Hello, World!")));
        let resp = service.serve(req).await?;
        assert_eq!(StatusCode::OK, resp.status);
        Ok(())
    }
}
