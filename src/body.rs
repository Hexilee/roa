use futures::io::{BufReader, Cursor};
use futures::task::{Context, Poll};
use futures::{AsyncBufRead as BufRead, AsyncBufReadExt, AsyncRead as Read, Stream};
use std::borrow::BorrowMut;
use std::io::Error;
use std::pin::Pin;

pub struct Body {
    counter: usize,
    segments: Vec<Segment>,
}

pub type Segment = Box<dyn BufRead + Sync + Send + Unpin + 'static>;

impl Body {
    pub fn new() -> Self {
        Self {
            counter: 0,
            segments: Vec::new(),
        }
    }

    pub fn write_buf(
        &mut self,
        buf_reader: impl BufRead + Sync + Send + Unpin + 'static,
    ) -> &mut Self {
        self.segments.push(Box::new(buf_reader));
        self
    }

    pub fn write(&mut self, reader: impl Read + Sync + Send + Unpin + 'static) -> &mut Self {
        self.write_buf(BufReader::new(reader))
    }

    pub fn write_bytes(&mut self, bytes: Vec<u8>) -> &mut Self {
        self.write_buf(Cursor::new(bytes))
    }

    pub fn write_str(&mut self, data: impl ToString) -> &mut Self {
        self.write_bytes(data.to_string().into_bytes())
    }

    pub fn stream(self) -> BodyStream<Self> {
        BodyStream::new(self)
    }
}

impl BufRead for Body {
    fn poll_fill_buf(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<&[u8], Error>> {
        let counter = self.counter;
        let mut_ref = self.get_mut();
        let mut buf = b"".as_ref();
        while buf.len() == 0 {
            if counter >= mut_ref.segments.len() {
                return Poll::Ready(Ok(buf));
            }
            let buf: &[u8] =
                futures::ready!(Pin::new(mut_ref.segments[counter].as_mut()).poll_fill_buf(cx))?;
        }
        Poll::Ready(Ok(buf))
    }

    fn consume(self: Pin<&mut Self>, amt: usize) {
        let self_mut = self.get_mut();
        let counter = self_mut.counter;
        if counter < self_mut.segments.len() {
            Pin::new(self_mut.segments[counter].as_mut()).consume(amt)
        }
    }
}

impl Read for Body {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<usize, Error>> {
        let data: &[u8] = futures::ready!(self.as_mut().poll_fill_buf(cx))?;
        let nums = std::cmp::min(data.len(), buf.len());
        buf.copy_from_slice(&data[0..nums]);
        self.consume(nums);
        Poll::Ready(Ok(nums))
    }
}

pub struct BodyStream<R: BufRead> {
    body: R,
}

impl<R: BufRead> BodyStream<R> {
    pub fn new(body: R) -> Self {
        Self { body }
    }
}

impl<R: BufRead + Unpin> futures::Stream for BodyStream<R> {
    type Item = Result<Vec<u8>, std::io::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let buf: &[u8] = futures::ready!(Pin::new(&mut self.body).poll_fill_buf(cx))?;
        let buf_len = buf.len();
        if buf_len == 0 {
            return Poll::Ready(None);
        } else {
            let data = buf.to_vec();
            Pin::new(&mut self.body).consume(buf_len);
            Poll::Ready(Some(Ok(data)))
        }
    }
}

impl<R: BufRead + Unpin + Send + Sync + 'static> From<BodyStream<R>> for hyper::Body {
    fn from(stream: BodyStream<R>) -> Self {
        hyper::Body::wrap_stream(stream)
    }
}
