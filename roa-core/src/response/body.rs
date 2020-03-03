use bytes::{Buf, Bytes, BytesMut};
use futures::future::ok;
use futures::io::{self, AsyncRead};
use futures::stream::{once, Stream};
use std::pin::Pin;
use std::task::{Context, Poll};

const DEFAULT_CHUNK_SIZE: usize = 4096;

/// The body of response.
pub struct Body {
    counter: usize,
    segments: Vec<Segment>,
}

type Segment = Box<dyn Stream<Item = io::Result<Bytes>> + Sync + Send + Unpin + 'static>;

impl Body {
    #[inline]
    pub(crate) fn new() -> Self {
        Self {
            counter: 0,
            segments: Vec::new(),
        }
    }

    /// Write reader with default chunk size.
    #[inline]
    pub fn write_stream(
        &mut self,
        stream: impl Stream<Item = io::Result<Bytes>> + Sync + Send + Unpin + 'static,
    ) -> &mut Self {
        self.segments.push(Box::new(stream));
        self
    }

    /// Write reader with default chunk size.
    #[inline]
    pub fn write_reader(
        &mut self,
        reader: impl AsyncRead + Sync + Send + Unpin + 'static,
    ) -> &mut Self {
        self.write_chunk(reader, DEFAULT_CHUNK_SIZE)
    }

    /// Write reader with chunk size.
    #[inline]
    pub fn write_chunk(
        &mut self,
        reader: impl AsyncRead + Sync + Send + Unpin + 'static,
        chunk_size: usize,
    ) -> &mut Self {
        self.write_stream(ReaderStream::new(reader, chunk_size))
    }

    /// Write `Bytes`.
    #[inline]
    pub fn write(&mut self, bytes: impl Into<Bytes>) -> &mut Self {
        self.write_stream(once(ok(bytes.into())))
    }

    /// Wrap self with a wrapper.
    #[inline]
    pub fn wrapped<S>(&mut self, wrapper: impl FnOnce(Self) -> S) -> &mut Self
    where
        S: Stream<Item = io::Result<Bytes>> + Sync + Send + Unpin + 'static,
    {
        let body = std::mem::take(self);
        self.write_stream(wrapper(body))
    }
}

impl Default for Body {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

pub struct ReaderStream<R> {
    chunk_size: usize,
    reader: R,
}

impl<R> ReaderStream<R> {
    #[inline]
    fn new(reader: R, chunk_size: usize) -> Self {
        Self { reader, chunk_size }
    }
}

impl<R> Stream for ReaderStream<R>
where
    R: AsyncRead + Unpin,
{
    type Item = io::Result<Bytes>;
    #[inline]
    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        let chunk_size = self.chunk_size;
        let mut chunk = BytesMut::with_capacity(chunk_size);
        unsafe { chunk.set_len(chunk_size) };
        let bytes =
            futures::ready!(Pin::new(&mut self.reader).poll_read(cx, &mut *chunk))?;
        if bytes == 0 {
            Poll::Ready(None)
        } else {
            Poll::Ready(Some(Ok(chunk.to_bytes().slice(0..bytes))))
        }
    }
}

impl Stream for Body {
    type Item = io::Result<Bytes>;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        if self.counter >= self.segments.len() {
            return Poll::Ready(None);
        }
        let counter = self.counter;
        match futures::ready!(Pin::new(&mut self.segments[counter]).poll_next(cx)) {
            None => {
                self.counter += 1;
                self.poll_next(cx)
            }
            some => Poll::Ready(some),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Body;
    use async_std::fs::File;
    use futures::AsyncReadExt;
    use futures::TryStreamExt;

    #[async_std::test]
    async fn body_empty() -> std::io::Result<()> {
        let body = Body::new();
        let mut data = String::new();
        body.into_async_read().read_to_string(&mut data).await?;
        assert_eq!("", data);
        Ok(())
    }

    #[async_std::test]
    async fn body_single() -> std::io::Result<()> {
        let mut body = Body::new();
        let mut data = String::new();
        body.write("Hello, World")
            .into_async_read()
            .read_to_string(&mut data)
            .await?;
        assert_eq!("Hello, World", data);
        Ok(())
    }

    #[async_std::test]
    async fn body_multiple() -> std::io::Result<()> {
        let mut body = Body::new();
        let mut data = String::new();
        body.write("He")
            .write("llo, ")
            .write("World")
            .into_async_read()
            .read_to_string(&mut data)
            .await?;
        assert_eq!("Hello, World", data);
        Ok(())
    }

    #[async_std::test]
    async fn body_composed() -> std::io::Result<()> {
        let mut body = Body::new();
        let mut data = String::new();
        body.write("He")
            .write("llo, ")
            .write_reader(File::open("../assets/author.txt").await?)
            .write(".")
            .into_async_read()
            .read_to_string(&mut data)
            .await?;
        assert_eq!("Hello, Hexilee.", data);
        Ok(())
    }
}
