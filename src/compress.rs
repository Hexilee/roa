use crate::core::header::CONTENT_ENCODING;
use crate::core::{async_trait, Body, Context, Error, Middleware, Next, Result, State, StatusCode};
use accept_encoding::{parse, Encoding};
use async_compression::flate2::Compression;
use async_compression::futures::bufread::{BrotliEncoder, GzipEncoder, ZlibEncoder, ZstdEncoder};
use std::sync::Arc;

#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
pub enum Compress {
    Fast = 0,
    Balance = 1,
    Best = 2,
}

impl Compress {
    fn compression(self) -> Compression {
        Compression::new((self as u32) * 4 + 1)
    }

    fn brotli_level(self) -> u32 {
        (self as u32) * 5 + 1
    }

    fn zstd_level(self) -> i32 {
        (self as i32) * 10 + 1
    }
}

#[async_trait]
impl<S: State> Middleware<S> for Compress {
    async fn handle(self: Arc<Self>, ctx: Context<S>, next: Next) -> Result {
        next().await?;
        let body: Body = std::mem::take(&mut *ctx.resp_mut().await);
        let content_encoding = match parse(&ctx.req().await.headers)
            .map_err(|err| Error::new(StatusCode::BAD_REQUEST, err, true))?
        {
            None | Some(Encoding::Gzip) => {
                ctx.resp_mut()
                    .await
                    .write(GzipEncoder::new(body, self.compression()));
                Encoding::Gzip.to_header_value()
            }
            Some(Encoding::Deflate) => {
                ctx.resp_mut()
                    .await
                    .write(ZlibEncoder::new(body, self.compression()));
                Encoding::Deflate.to_header_value()
            }
            Some(Encoding::Brotli) => {
                ctx.resp_mut()
                    .await
                    .write(BrotliEncoder::new(body, self.brotli_level()));
                Encoding::Brotli.to_header_value()
            }
            Some(Encoding::Zstd) => {
                ctx.resp_mut()
                    .await
                    .write(ZstdEncoder::new(body, self.zstd_level()));
                Encoding::Zstd.to_header_value()
            }
            Some(Encoding::Identity) => {
                ctx.resp_mut().await.write_buf(body);
                Encoding::Identity.to_header_value()
            }
        };
        ctx.resp_mut()
            .await
            .headers
            .append(CONTENT_ENCODING, content_encoding);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::Compress;

    #[test]
    fn fast() {
        let level = Compress::Fast;
        assert_eq!(1, level.compression().level());
        assert_eq!(1, level.brotli_level());
        assert_eq!(1, level.zstd_level());
    }

    #[test]
    fn balance() {
        let level = Compress::Balance;
        assert_eq!(5, level.compression().level());
        assert_eq!(6, level.brotli_level());
        assert_eq!(11, level.zstd_level());
    }

    #[test]
    fn best() {
        let level = Compress::Best;
        assert_eq!(9, level.compression().level());
        assert_eq!(11, level.brotli_level());
        assert_eq!(21, level.zstd_level());
    }
}
