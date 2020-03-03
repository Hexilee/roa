use crate::Body;
use futures::stream::TryStreamExt;
use futures::AsyncRead;
use http::{HeaderMap, HeaderValue, Method, Uri, Version};
use std::io;
use std::ops::{Deref, DerefMut};

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
    /// Get body.
    /// This method will consume inner body.
    #[inline]
    pub fn stream(&mut self) -> Body {
        std::mem::take(&mut self.body)
    }

    /// Get body as AsyncRead.
    /// This method will consume inner body.
    #[inline]
    pub fn body(&mut self) -> impl AsyncRead + Sync + Send + Unpin + 'static {
        self.stream().into_async_read()
    }
}

impl Deref for Request {
    type Target = Body;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.body
    }
}

impl DerefMut for Request {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.body
    }
}

impl From<http::Request<hyper::Body>> for Request {
    #[inline]
    fn from(req: http::Request<hyper::Body>) -> Self {
        let (parts, hyper_body) = req.into_parts();
        let mut body = Body::default();
        body.write_stream(
            hyper_body.map_err(|err| io::Error::new(io::ErrorKind::Other, err)),
        );
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
        http::Request::new(hyper::Body::empty()).into()
    }
}

#[cfg(all(test, feature = "runtime"))]
mod tests {
    use crate::{App, Request};
    use futures::AsyncReadExt;
    use http::StatusCode;

    #[async_std::test]
    async fn body_read() -> Result<(), Box<dyn std::error::Error>> {
        let mut app = App::new(());
        app.gate_fn(|mut ctx, _next| async move {
            let mut data = String::new();
            ctx.req_mut().body().read_to_string(&mut data).await?;
            assert_eq!("Hello, World!", data);
            Ok(())
        });
        let service = app.fake_service();
        let mut req = Request::default();
        req.write("Hello, World!");
        let resp = service.serve(req).await?;
        assert_eq!(StatusCode::OK, resp.status);
        Ok(())
    }
}
