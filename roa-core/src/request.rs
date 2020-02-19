use futures::stream::TryStreamExt;
use futures::AsyncRead;
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
    /// Get inner hyper body.
    /// This method will consume inner body.
    pub fn body_stream(&mut self) -> Body {
        let mut body = Body::empty();
        std::mem::swap(&mut body, &mut self.body);
        body
    }

    /// Get inner as AsyncRead.
    /// This method will consume inner body.
    pub fn body(&mut self) -> impl AsyncRead + Sync + Send + Unpin + 'static {
        self.body_stream()
            .map_err(|err| io::Error::new(io::ErrorKind::Other, err))
            .into_async_read()
    }
}

impl From<http::Request<Body>> for Request {
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
    fn default() -> Self {
        http::Request::new(Body::empty()).into()
    }
}

#[cfg(test)]
mod tests {
    use crate::App;
    use async_std::task::spawn;
    use futures::AsyncReadExt;
    use http::StatusCode;

    #[tokio::test]
    async fn body_read() -> Result<(), Box<dyn std::error::Error>> {
        let mut app = App::new(());
        app.gate_fn(|mut ctx, _next| async move {
            let mut data = String::new();
            ctx.req_mut().body().read_to_string(&mut data).await?;
            assert_eq!("Hello, World!", data);
            Ok(())
        });
        let (addr, server) = app.run_local()?;
        spawn(server);
        let client = reqwest::Client::new();
        let resp = client
            .post(&format!("http://{}", addr))
            .body("Hello, World!")
            .send()
            .await?;
        assert_eq!(StatusCode::OK, resp.status());
        Ok(())
    }
}
