//! The compress module of roa.
//! This module provides a middleware `Compress`.
//!
//! ### Example
//!
//! ```rust
//! use roa::compress::{Compress, Level};
//! use roa::body::DispositionType::*;
//! use roa::App;
//! use roa::preload::*;
//!
//!
//! # fn main() -> std::io::Result<()> {
//! let (addr, server) = App::new(())
//!     .gate(Compress(Level::Fastest))
//!     .end(|mut ctx| async move {
//!         ctx.write_file("../assets/welcome.html", Inline).await
//!     })
//!     .run()?;
//! // server.await
//! Ok(())
//! # }
//! ```

pub use async_compression::Level;

use crate::http::{header::CONTENT_ENCODING, StatusCode};
use crate::{async_trait, Context, Error, Middleware, Next, Result, State};
use accept_encoding::{parse, Encoding};
use async_compression::stream::{BrotliEncoder, GzipEncoder, ZlibEncoder, ZstdEncoder};
use std::sync::Arc;

/// A middleware to negotiate with client and compress response body automatically,
/// supports gzip, deflate, brotli, zstd and identity.
#[derive(Debug, Copy, Clone)]
pub struct Compress(pub Level);

impl Default for Compress {
    fn default() -> Self {
        Self(Level::Default)
    }
}

#[async_trait(?Send)]
impl<S: State> Middleware<S> for Compress {
    async fn handle(self: Arc<Self>, mut ctx: Context<S>, next: Next) -> Result {
        next.await?;
        let level = self.0;
        let best_encoding = parse(&ctx.req().headers)
            .map_err(|err| Error::new(StatusCode::BAD_REQUEST, err, true))?;
        let content_encoding = match best_encoding {
            None | Some(Encoding::Gzip) => {
                ctx.resp_mut()
                    .map_body(move |body| GzipEncoder::with_quality(body, level));
                Encoding::Gzip.to_header_value()
            }
            Some(Encoding::Deflate) => {
                ctx.resp_mut()
                    .map_body(move |body| ZlibEncoder::with_quality(body, level));
                Encoding::Deflate.to_header_value()
            }
            Some(Encoding::Brotli) => {
                ctx.resp_mut()
                    .map_body(move |body| BrotliEncoder::with_quality(body, level));
                Encoding::Brotli.to_header_value()
            }
            Some(Encoding::Zstd) => {
                ctx.resp_mut()
                    .map_body(move |body| ZstdEncoder::with_quality(body, level));
                Encoding::Zstd.to_header_value()
            }
            Some(Encoding::Identity) => Encoding::Identity.to_header_value(),
        };
        ctx.resp_mut()
            .headers
            .append(CONTENT_ENCODING, content_encoding);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::body::DispositionType::*;
    use crate::compress::{Compress, Level};
    use crate::http::{header::ACCEPT_ENCODING, StatusCode};
    use crate::preload::*;
    use crate::{App, Context, Middleware, Next};
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
    fn assert_consumed(assert_counter: usize) -> impl Middleware<()> {
        move |mut ctx: Context<()>, next: Next| async move {
            next.await?;
            ctx.resp_mut().map_body(move |stream| Consumer {
                counter: 0,
                stream,
                assert_counter,
            });
            Ok(())
        }
    }

    #[tokio::test]
    async fn compress() -> Result<(), Box<dyn std::error::Error>> {
        let (addr, server) = App::new(())
            .gate(assert_consumed(202)) // compressed
            .gate(Compress(Level::Fastest))
            .gate(assert_consumed(236)) // the size of assets/welcome.html is 236 bytes.
            .end(|mut ctx| async move {
                ctx.write_file("../assets/welcome.html", Inline).await
            })
            .run()?;
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
