//! The compress module of roa.
//! This module provides a middleware `Compress`.
//!
//! ### Example
//!
//! ```rust
//! use roa::compress::{Compress, Level};
//! use roa::body::PowerBody;
//! use roa::core::{App, StatusCode, header::ACCEPT_ENCODING};
//! use async_std::task::spawn;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     pretty_env_logger::init();
//!     let (addr, server) = App::new(())
//!         .gate_fn(|mut ctx, next| async move {
//!             next.await?;
//!             // compress body to 202 bytes in gzip with quantity Level::Fastest.
//!             ctx.resp_mut().on_finish(|body| assert_eq!(202, body.consumed()));
//!             Ok(())
//!         })
//!         .gate(Compress(Level::Fastest))
//!         .end(|mut ctx| async move {
//!             // the size of assets/welcome.html is 236 bytes.
//!             ctx.resp_mut().on_finish(|body| assert_eq!(236, body.consumed()));
//!             ctx.write_file("assets/welcome.html").await
//!         })
//!         .run_local()?;
//!     spawn(server);
//!     let client = reqwest::Client::builder().gzip(true).build()?;
//!     let resp = client
//!         .get(&format!("http://{}", addr))
//!         .header(ACCEPT_ENCODING, "gzip")
//!         .send()
//!         .await?;
//!     assert_eq!(StatusCode::OK, resp.status());
//!     Ok(())
//! }
//! ```
pub use async_compression::Level;

use crate::core::header::CONTENT_ENCODING;
use crate::core::{
    async_trait, Context, Error, Middleware, Next, Result, State, StatusCode,
};
use accept_encoding::{parse, Encoding};
use async_compression::futures::bufread::{
    BrotliEncoder, GzipEncoder, ZlibEncoder, ZstdEncoder,
};
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
        let body = std::mem::take(&mut **ctx.resp_mut());
        let best_encoding = parse(&ctx.req().headers)
            .map_err(|err| Error::new(StatusCode::BAD_REQUEST, err, true))?;
        let content_encoding = match best_encoding {
            None | Some(Encoding::Gzip) => {
                ctx.resp_mut()
                    .write(GzipEncoder::with_quality(body, self.0));
                Encoding::Gzip.to_header_value()
            }
            Some(Encoding::Deflate) => {
                ctx.resp_mut()
                    .write(ZlibEncoder::with_quality(body, self.0));
                Encoding::Deflate.to_header_value()
            }
            Some(Encoding::Brotli) => {
                ctx.resp_mut()
                    .write(BrotliEncoder::with_quality(body, self.0));
                Encoding::Brotli.to_header_value()
            }
            Some(Encoding::Zstd) => {
                ctx.resp_mut()
                    .write(ZstdEncoder::with_quality(body, self.0));
                Encoding::Zstd.to_header_value()
            }
            Some(Encoding::Identity) => {
                ctx.resp_mut().write_buf(body);
                Encoding::Identity.to_header_value()
            }
        };
        ctx.resp_mut()
            .headers
            .append(CONTENT_ENCODING, content_encoding);
        Ok(())
    }
}
