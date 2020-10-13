//! This module provides a middleware `Compress`.
//!
//! ### Example
//!
//! ```rust
//! use roa::compress::{Compress, Level};
//! use roa::body::DispositionType::*;
//! use roa::{App, Context};
//! use roa::preload::*;
//! use std::error::Error;
//!
//! async fn end(ctx: &mut Context) -> roa::Result {
//!     ctx.write_file("../assets/welcome.html", Inline).await
//! }
//!
//! # fn main() -> Result<(), Box<dyn Error>> {
//! let mut app = App::new().gate(Compress(Level::Fastest)).end(end);
//! let (addr, server) = app.run()?;
//! // server.await
//! Ok(())
//! # }
//! ```

pub use async_compression::Level;

use crate::http::header::{HeaderMap, ACCEPT_ENCODING, CONTENT_ENCODING};
use crate::http::{HeaderValue, StatusCode};
use crate::{async_trait, status, Context, Middleware, Next, Result};

use async_compression::stream::{BrotliEncoder, GzipEncoder, ZlibEncoder, ZstdEncoder};

/// A middleware to negotiate with client and compress response body automatically,
/// supports gzip, deflate, brotli, zstd and identity.
#[derive(Debug, Copy, Clone)]
pub struct Compress(pub Level);

/// Encodings to use.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum Encoding {
    /// The Gzip encoding.
    Gzip,
    /// The Deflate encoding.
    Deflate,
    /// The Brotli encoding.
    Brotli,
    /// The Zstd encoding.
    Zstd,
    /// No encoding.
    Identity,
}

impl Encoding {
    /// Parses a given string into its corresponding encoding.
    fn parse(s: &str) -> Result<Option<Encoding>> {
        match s {
            "gzip" => Ok(Some(Encoding::Gzip)),
            "deflate" => Ok(Some(Encoding::Deflate)),
            "br" => Ok(Some(Encoding::Brotli)),
            "zstd" => Ok(Some(Encoding::Zstd)),
            "identity" => Ok(Some(Encoding::Identity)),
            "*" => Ok(None),
            _ => Err(status!(
                StatusCode::BAD_REQUEST,
                format!("unknown encoding: {}", s),
                true
            )),
        }
    }

    /// Converts the encoding into its' corresponding header value.
    fn to_header_value(self) -> HeaderValue {
        match self {
            Encoding::Gzip => HeaderValue::from_str("gzip").unwrap(),
            Encoding::Deflate => HeaderValue::from_str("deflate").unwrap(),
            Encoding::Brotli => HeaderValue::from_str("br").unwrap(),
            Encoding::Zstd => HeaderValue::from_str("zstd").unwrap(),
            Encoding::Identity => HeaderValue::from_str("identity").unwrap(),
        }
    }
}

fn select_encoding(headers: &HeaderMap) -> Result<Option<Encoding>> {
    let mut preferred_encoding = None;
    let mut max_qval = 0.0;

    for (encoding, qval) in accept_encodings(headers)? {
        if qval == 1.0 {
            preferred_encoding = encoding;
            break;
        } else if qval > max_qval {
            preferred_encoding = encoding;
            max_qval = qval;
        }
    }
    Ok(preferred_encoding)
}

/// Parse a set of HTTP headers into a vector containing tuples of options containing encodings and their corresponding q-values.
///
/// If you're looking for more fine-grained control over what encoding to choose for the client, or if you don't support every [`Encoding`] listed, this is likely what you want.
///
/// Note that a result of `None` indicates there preference is expressed on which encoding to use.
/// Either the `Accept-Encoding` header is not present, or `*` is set as the most preferred encoding.
fn accept_encodings(headers: &HeaderMap) -> Result<Vec<(Option<Encoding>, f32)>> {
    headers
        .get_all(ACCEPT_ENCODING)
        .iter()
        .map(|hval| {
            hval.to_str()
                .map_err(|err| status!(StatusCode::BAD_REQUEST, err, true))
        })
        .collect::<Result<Vec<&str>>>()?
        .iter()
        .flat_map(|s| s.split(',').map(str::trim))
        .filter_map(|v| {
            let pair: Vec<&str> = v.splitn(2, ";q=").collect();
            if pair.len() == 0 {
                return None;
            }

            let encoding = match Encoding::parse(pair[0]) {
                Ok(encoding) => encoding,
                Err(_) => return None, // ignore unknown encodings
            };

            let qval = if pair.len() == 1 {
                1.0
            } else {
                match pair[1].parse::<f32>() {
                    Ok(f) => f,
                    Err(err) => {
                        return Some(Err(status!(StatusCode::BAD_REQUEST, err, true)))
                    }
                }
            };
            Some(Ok((encoding, qval)))
        })
        .collect::<Result<Vec<(Option<Encoding>, f32)>>>()
}

impl Default for Compress {
    fn default() -> Self {
        Self(Level::Default)
    }
}

#[async_trait(?Send)]
impl<'a, S> Middleware<'a, S> for Compress {
    #[allow(clippy::trivially_copy_pass_by_ref)]
    #[inline]
    async fn handle(&'a self, ctx: &'a mut Context<S>, next: Next<'a>) -> Result {
        next.await?;
        let level = self.0;
        let best_encoding = select_encoding(&ctx.req.headers)?;
        let body = std::mem::take(&mut ctx.resp.body);
        let content_encoding = match best_encoding {
            None | Some(Encoding::Gzip) => {
                ctx.resp
                    .write_stream(GzipEncoder::with_quality(body, level));
                Encoding::Gzip.to_header_value()
            }
            Some(Encoding::Deflate) => {
                ctx.resp
                    .write_stream(ZlibEncoder::with_quality(body, level));
                Encoding::Deflate.to_header_value()
            }
            Some(Encoding::Brotli) => {
                ctx.resp
                    .write_stream(BrotliEncoder::with_quality(body, level));
                Encoding::Brotli.to_header_value()
            }
            Some(Encoding::Zstd) => {
                ctx.resp
                    .write_stream(ZstdEncoder::with_quality(body, level));
                Encoding::Zstd.to_header_value()
            }
            Some(Encoding::Identity) => {
                ctx.resp.body = body;
                Encoding::Identity.to_header_value()
            }
        };
        ctx.resp.headers.append(CONTENT_ENCODING, content_encoding);
        Ok(())
    }
}

#[cfg(all(test, feature = "tcp", feature = "file"))]
mod tests {
    use crate::body::DispositionType::*;
    use crate::compress::{Compress, Level};
    use crate::http::{header::ACCEPT_ENCODING, StatusCode};
    use crate::preload::*;
    use crate::{async_trait, App, Context, Middleware, Next};
    use async_std::task::spawn;
    use bytes::Bytes;
    use futures::Stream;
    use std::io;
    use std::pin::Pin;
    use std::task::{self, Poll};

    struct Consumer<S> {
        counter: usize,
        stream: S,
        assert_counter: usize,
    }
    impl<S> Stream for Consumer<S>
    where
        S: 'static + Send + Send + Unpin + Stream<Item = io::Result<Bytes>>,
    {
        type Item = io::Result<Bytes>;
        fn poll_next(
            mut self: Pin<&mut Self>,
            cx: &mut task::Context<'_>,
        ) -> Poll<Option<Self::Item>> {
            match Pin::new(&mut self.stream).poll_next(cx) {
                Poll::Ready(Some(Ok(bytes))) => {
                    self.counter += bytes.len();
                    Poll::Ready(Some(Ok(bytes)))
                }
                Poll::Ready(None) => {
                    assert_eq!(self.assert_counter, self.counter);
                    Poll::Ready(None)
                }
                poll => poll,
            }
        }
    }

    struct Assert(usize);

    #[async_trait(?Send)]
    impl<'a, S> Middleware<'a, S> for Assert {
        async fn handle(
            &'a self,
            ctx: &'a mut Context<S>,
            next: Next<'a>,
        ) -> crate::Result {
            next.await?;
            let body = std::mem::take(&mut ctx.resp.body);
            ctx.resp.write_stream(Consumer {
                counter: 0,
                stream: body,
                assert_counter: self.0,
            });
            Ok(())
        }
    }

    async fn end(ctx: &mut Context) -> crate::Result {
        ctx.write_file("../assets/welcome.html", Inline).await
    }

    #[tokio::test]
    async fn compress() -> Result<(), Box<dyn std::error::Error>> {
        let app = App::new()
            .gate(Assert(202)) // compressed to 202 bytes
            .gate(Compress(Level::Fastest))
            .gate(Assert(236)) // the size of assets/welcome.html is 236 bytes.
            .end(end);
        let (addr, server) = app.run()?;
        spawn(server);
        let client = reqwest::Client::builder().gzip(true).build()?;
        let resp = client
            .get(&format!("http://{}", addr))
            .header(ACCEPT_ENCODING, "gzip")
            .send()
            .await?;
        assert_eq!(StatusCode::OK, resp.status());
        assert_eq!(236, resp.text().await?.len());
        Ok(())
    }
}
