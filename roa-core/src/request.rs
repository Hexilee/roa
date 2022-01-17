use std::io;

use bytes::Bytes;
use futures::stream::{Stream, TryStreamExt};
use http::{Extensions, HeaderMap, HeaderValue, Method, Uri, Version};
use hyper::Body;
use tokio::io::AsyncRead;
use tokio_util::io::StreamReader;
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

    extensions: Extensions,

    body: Body,
}

impl Request {
    /// Take raw hyper request.
    /// This method will consume inner body and extensions.
    #[inline]
    pub fn take_raw(&mut self) -> http::Request<Body> {
        let mut builder = http::Request::builder()
            .method(self.method.clone())
            .uri(self.uri.clone());
        *builder.extensions_mut().expect("fail to get extensions") =
            std::mem::take(&mut self.extensions);
        *builder.headers_mut().expect("fail to get headers") = self.headers.clone();
        builder
            .body(self.raw_body())
            .expect("fail to build raw body")
    }

    /// Gake raw hyper body.
    /// This method will consume inner body.
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
        StreamReader::new(self.stream())
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
            extensions: parts.extensions,
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
    use http::StatusCode;
    use hyper::Body;
    use tokio::io::AsyncReadExt;

    use crate::{App, Context, Request, Status};

    async fn test(ctx: &mut Context) -> Result<(), Status> {
        let mut data = String::new();
        ctx.req.reader().read_to_string(&mut data).await?;
        assert_eq!("Hello, World!", data);
        Ok(())
    }

    #[tokio::test]
    async fn body_read() -> Result<(), Box<dyn std::error::Error>> {
        let app = App::new().end(test);
        let service = app.http_service();
        let req = Request::from(http::Request::new(Body::from("Hello, World!")));
        let resp = service.serve(req).await;
        assert_eq!(StatusCode::OK, resp.status);
        Ok(())
    }
}
