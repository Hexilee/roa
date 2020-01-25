use bytes::Bytes;
use futures::task::{Context, Poll};
use futures::{AsyncRead, Stream};
use http::response::Builder;
use std::pin::Pin;

pub struct Body {
    segments: Box<dyn Iterator<Item = Segment> + Sync + Send + 'static>,
}

pub type Segment = Pin<Box<dyn AsyncRead + Sync + Send + 'static>>;

pub struct Response {
    builder: Builder,
    segments: Vec<Segment>,
}

impl Response {
    pub fn new() -> Self {
        Self {
            builder: Builder::new(),
            segments: Vec::new(),
        }
    }

    pub fn into_resp(self) -> Result<http_service::Response, http::Error> {
        let Self { builder, segments } = self;
        unimplemented!()
    }
}

impl Body {
    pub fn new(segments: impl Iterator<Item = Segment> + Sync + Send + 'static) -> Self {
        Self {
            segments: Box::new(segments),
        }
    }
}

// TODO: complete it
impl Stream for Body {
    type Item = std::io::Result<Bytes>;
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        unimplemented!()
    }
}
