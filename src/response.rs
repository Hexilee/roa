use async_std::io::BufReader;
use futures::task::{Context, Poll};
use futures::AsyncRead as Read;
use http::response::Builder;
use std::io::Error;
use std::pin::Pin;

pub struct Body {
    counter: usize,
    segments: Vec<Segment>,
}

pub type Segment = Pin<Box<dyn Read + Send + 'static>>;

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
        let Self {
            mut builder,
            segments,
        } = self;
        builder.body(http_service::Body::from_reader(BufReader::new(Body::new(
            segments,
        ))))
    }
}

impl Body {
    pub fn new(segments: Vec<Segment>) -> Self {
        Self {
            counter: 0,
            segments,
        }
    }
}

// TODO: complete it
impl Read for Body {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<usize, Error>> {
        let self_mut = self.get_mut();
        let counter = self_mut.counter;
        if counter >= self_mut.segments.len() {
            return Poll::Ready(Ok(0));
        }
        let reader = self_mut.segments[counter].as_mut();
        match reader.poll_read(cx, buf) {
            Poll::Ready(Ok(0)) => {
                self_mut.counter += 1;
                Pin::new(self_mut).poll_read(cx, buf)
            }
            ret => ret,
        }
    }
}
