use std::{io, mem};

use bytes::Bytes;
use futures::stream::TryStreamExt;
use futures::{AsyncRead, Stream};
use http::request::Parts;
use http::{HeaderMap, HeaderValue, Method, StatusCode, Uri, Version};
use hyper::upgrade::{self, OnUpgrade};
use hyper::Body;

use crate::throw;

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

    // parts of origin request, for upgrade protocol
    parts: Option<Parts>,

    body: Body,
}

impl Request {
    /// Get raw hyper body.
    #[inline]
    pub fn raw_body(&mut self) -> Body {
        mem::take(&mut self.body)
    }

    /// Upgrade protocol
    #[inline]
    pub fn on_upgrade(&mut self) -> crate::Result<OnUpgrade> {
        let parts = match self.parts.take() {
            None => throw!(
                StatusCode::INTERNAL_SERVER_ERROR,
                "each request can only be upgraded once",
                false
            ),
            Some(p) => p,
        };
        Ok(upgrade::on(http::Request::from_parts(parts, Body::empty())))
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
            method: parts.method.clone(),
            uri: parts.uri.clone(),
            version: parts.version,
            headers: parts.headers.clone(),
            parts: Some(parts),
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
    use futures::AsyncReadExt;
    use http::StatusCode;
    use hyper::Body;

    use crate::{App, Context, Request, Status};

    async fn test(ctx: &mut Context) -> Result<(), Status> {
        let mut data = String::new();
        ctx.req.reader().read_to_string(&mut data).await?;
        assert_eq!("Hello, World!", data);
        Ok(())
    }

    #[async_std::test]
    async fn body_read() -> Result<(), Box<dyn std::error::Error>> {
        let app = App::new().end(test);
        let service = app.http_service();
        let req = Request::from(http::Request::new(Body::from("Hello, World!")));
        let resp = service.serve(req).await;
        assert_eq!(StatusCode::OK, resp.status);
        Ok(())
    }
}
