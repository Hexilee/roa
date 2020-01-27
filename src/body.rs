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

    pub fn write_bytes(&mut self, bytes: impl Into<Vec<u8>>) -> &mut Self {
        self.write_buf(Cursor::new(bytes.into()))
    }

    pub fn write_str(&mut self, data: impl ToString) -> &mut Self {
        self.write_bytes(data.to_string())
    }

    pub fn stream(self) -> BodyStream<Self> {
        BodyStream::new(self)
    }

    pub fn poll_segment(&mut self, cx: &mut Context<'_>) -> Poll<Result<&[u8], Error>> {
        Pin::new(self.segments[self.counter].as_mut()).poll_fill_buf(cx)
    }
}

impl BufRead for Body {
    fn poll_fill_buf(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<&[u8], Error>> {
        let mut_ref = self.get_mut();
        let mut buf_len = 0;
        let mut buf_ptr = b"".as_ref() as *const [u8];
        loop {
            if mut_ref.counter >= mut_ref.segments.len() {
                break Poll::Ready(Ok(b"".as_ref()));
            }
            let buf = futures::ready!(mut_ref.poll_segment(cx))?;
            buf_len = buf.len();
            buf_ptr = buf as *const [u8];
            if buf_len != 0 {
                break Poll::Ready(Ok(unsafe { &*buf_ptr }));
            }
            mut_ref.counter += 1;
        }
    }

    fn consume(self: Pin<&mut Self>, amt: usize) {
        let self_mut = self.get_mut();
        if self_mut.counter < self_mut.segments.len() {
            Pin::new(self_mut.segments[self_mut.counter].as_mut()).consume(amt)
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
        buf[0..nums].copy_from_slice(&data[0..nums]);
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

#[cfg(test)]
mod tests {
    use super::Body;
    use async_std::fs::File;
    use futures::AsyncReadExt;

    #[async_std::test]
    async fn body_empty() -> std::io::Result<()> {
        let mut body = Body::new();
        let mut data = String::new();
        body.read_to_string(&mut data).await?;
        assert_eq!("", data);
        Ok(())
    }

    #[async_std::test]
    async fn body_single() -> std::io::Result<()> {
        let mut body = Body::new();
        let mut data = String::new();
        body.write_buf(b"Hello, World".as_ref())
            .read_to_string(&mut data)
            .await?;
        println!("{}", &data);
        assert_eq!("Hello, World", data);
        Ok(())
    }

    #[async_std::test]
    async fn body_multiple() -> std::io::Result<()> {
        let mut body = Body::new();
        let mut data = String::new();
        body.write_buf(b"He".as_ref())
            .write_buf(b"llo, ".as_ref())
            .write_buf(b"World".as_ref())
            .read_to_string(&mut data)
            .await?;
        assert_eq!("Hello, World", data);
        Ok(())
    }

    #[async_std::test]
    async fn body_composed() -> std::io::Result<()> {
        let mut body = Body::new();
        let mut data = String::new();
        body.write_buf(b"He".as_ref())
            .write_buf(b"llo, ".as_ref())
            .write(File::open("assets/test_data.txt").await?)
            .write_buf(b".".as_ref())
            .read_to_string(&mut data)
            .await?;
        assert_eq!("Hello, Hexilee.", data);
        Ok(())
    }

    #[async_std::test]
    async fn response_write_str() -> std::io::Result<()> {
        let mut body = Body::new();
        let mut data = String::new();
        body.write_str("Hello, World")
            .read_to_string(&mut data)
            .await?;
        assert_eq!("Hello, World", data);
        Ok(())
    }
}
