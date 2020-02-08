use crate::core::{Body, Context, DynTargetHandler, Error, Model, Next, TargetHandler};
use accept_encoding::{parse, Encoding};
use async_compression::flate2::Compression;
use async_compression::futures::bufread::{BrotliEncoder, GzipEncoder, ZlibEncoder, ZstdEncoder};
use http::header::CONTENT_ENCODING;
use http::StatusCode;

#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
pub enum Level {
    Fast = 0,
    Balance = 1,
    Best = 2,
}

impl Level {
    fn to_compression(&self) -> Compression {
        Compression::new((*self as u32) * 4 + 1)
    }

    fn to_brotli_level(&self) -> u32 {
        (*self as u32) * 5 + 1
    }

    fn to_zstd_level(&self) -> i32 {
        (*self as i32) * 10 + 1
    }
}

pub fn compress<M: Model>(level: Level) -> Box<DynTargetHandler<M, Next>> {
    Box::new(move |ctx: Context<M>, next: Next| async move {
        next().await?;
        let body: Body = std::mem::take(&mut *ctx.resp_mut().await);
        let content_encoding = match parse(&ctx.req().await.headers)
            .map_err(|err| Error::new(StatusCode::BAD_REQUEST, err, true))?
        {
            None | Some(Encoding::Gzip) => {
                ctx.resp_mut()
                    .await
                    .write(GzipEncoder::new(body, level.to_compression()));
                Encoding::Gzip.to_header_value()
            }
            Some(Encoding::Deflate) => {
                ctx.resp_mut()
                    .await
                    .write(ZlibEncoder::new(body, level.to_compression()));
                Encoding::Deflate.to_header_value()
            }
            Some(Encoding::Brotli) => {
                ctx.resp_mut()
                    .await
                    .write(BrotliEncoder::new(body, level.to_brotli_level()));
                Encoding::Brotli.to_header_value()
            }
            Some(Encoding::Zstd) => {
                ctx.resp_mut()
                    .await
                    .write(ZstdEncoder::new(body, level.to_zstd_level()));
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
    })
    .dynamic()
}

#[cfg(test)]
mod tests {
    use super::Level;

    #[test]
    fn fast() {
        let level = Level::Fast;
        assert_eq!(1, level.to_compression().level());
        assert_eq!(1, level.to_brotli_level());
        assert_eq!(1, level.to_zstd_level());
    }

    #[test]
    fn balance() {
        let level = Level::Balance;
        assert_eq!(5, level.to_compression().level());
        assert_eq!(6, level.to_brotli_level());
        assert_eq!(11, level.to_zstd_level());
    }

    #[test]
    fn best() {
        let level = Level::Best;
        assert_eq!(9, level.to_compression().level());
        assert_eq!(11, level.to_brotli_level());
        assert_eq!(21, level.to_zstd_level());
    }
}
