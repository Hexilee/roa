use crate::{Body, Context, DynTargetHandler, Model, Next, TargetHandler};
pub use async_compression::flate2::Compression;
use async_compression::futures::bufread::{GzipEncoder, ZlibEncoder};
use async_std::io::Read;
use http::header::{HeaderValue, CONTENT_ENCODING};
use std::sync::Arc;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum Algorithm {
    Gzip,
    Deflate,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Options {
    pub algorithm: Algorithm,
    pub level: Compression,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            algorithm: Algorithm::Gzip,
            level: Compression::new(5),
        }
    }
}

pub fn compress<M: Model>(options: Options) -> Box<DynTargetHandler<M, Next>> {
    let compresser: Arc<
        dyn 'static + Send + Sync + Fn(Body) -> Box<dyn Read + Sync + Send + Unpin + 'static>,
    > = match options.algorithm {
        Algorithm::Gzip => Arc::new(move |body| Box::new(GzipEncoder::new(body, options.level))),
        Algorithm::Deflate => Arc::new(move |body| Box::new(ZlibEncoder::new(body, options.level))),
    };

    let content_encoding: HeaderValue = match options.algorithm {
        Algorithm::Gzip => "gzip",
        Algorithm::Deflate => "deflate",
    }
    .parse()
    .unwrap_or_else(|err| {
        panic!(format!(
            r"{}\nThis is a bug of roa::compress.
    Please report it to https://github.com/roa",
            err
        ))
    });
    Box::new(move |ctx: Context<M>, next: Next| {
        let compresser = compresser.clone();
        let content_encoding = content_encoding.clone();
        async move {
            next().await?;
            let body: Body = std::mem::take(&mut *ctx.resp_mut().await);
            ctx.resp_mut().await.write(compresser(body));
            ctx.resp_mut().await.headers.insert(CONTENT_ENCODING, content_encoding);
            Ok(())
        }
    })
    .dynamic()
}
