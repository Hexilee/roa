use bytes::{Buf, Bytes, BytesMut};
use futures::future::ok;
use futures::io::{self, AsyncRead};
use futures::stream::{once, Stream};
use std::mem;
use std::pin::Pin;
use std::task::{Context, Poll};

const DEFAULT_CHUNK_SIZE: usize = 4096;

/// The body of response.
///
/// ### Example
///
/// ```rust
/// use roa_core::Body;
/// use futures::StreamExt;
/// use std::io;
/// use bytes::Bytes;
///
/// async fn read_body(body: Body) -> io::Result<Bytes> {
///     Ok(match body {
///         Body::Empty => Bytes::new(),
///         Body::Once(bytes) => bytes,
///         Body::Stream(mut stream) => {
///             let mut bytes = Vec::new();
///             while let Some(item) = stream.next().await {
///                 bytes.extend_from_slice(&*item?);
///             }
///             bytes.into()
///         }
///     })
/// }
/// ```
pub enum Body {
    /// Empty kind
    Empty,

    /// Bytes kind.
    Once(Bytes),

    /// Stream kind.
    Stream(BodyStream),
}

/// Stream based body.
#[derive(Default)]
pub struct BodyStream {
    counter: usize,
    segments: Vec<Segment>,
}

type Segment = Box<dyn Stream<Item = io::Result<Bytes>> + Sync + Send + Unpin + 'static>;

impl Body {
    /// Construct an empty body.
    #[inline]
    pub fn empty() -> Self {
        Body::Empty
    }

    /// Construct a once body.
    #[inline]
    pub fn once(bytes: impl Into<Bytes>) -> Self {
        Body::Once(bytes.into())
    }

    /// Construct an empty body of stream kind.
    #[inline]
    pub fn stream() -> Self {
        Body::Stream(BodyStream {
            counter: 0,
            segments: Vec::new(),
        })
    }

    /// Write stream.
    pub fn write_stream(
        &mut self,
        stream: impl Stream<Item = io::Result<Bytes>> + Sync + Send + Unpin + 'static,
    ) -> &mut Self {
        match self {
            Body::Empty => {
                *self = Self::stream();
                self.write_stream(stream)
            }
            Body::Once(bytes) => {
                let data = mem::take(bytes);
                *self = Self::stream();
                self.write(data);
                self.write_stream(stream)
            }
            Body::Stream(body_stream) => {
                body_stream.write_stream(stream);
                self
            }
        }
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
    pub fn write(&mut self, data: impl Into<Bytes>) -> &mut Self {
        match self {
            Body::Empty => {
                *self = Self::once(data.into());
            }
            Body::Once(bytes) => {
                let mut stream = Self::stream();
                stream.write(mem::take(bytes));
                stream.write(data.into());
                *self = stream;
            }
            Body::Stream(stream) => stream.write(data),
        }
        self
    }
}

impl BodyStream {
    /// Write stream.
    #[inline]
    fn write_stream(
        &mut self,
        stream: impl Stream<Item = io::Result<Bytes>> + Sync + Send + Unpin + 'static,
    ) {
        self.segments.push(Box::new(stream))
    }

    #[inline]
    fn write(&mut self, bytes: impl Into<Bytes>) {
        self.write_stream(once(ok(bytes.into())))
    }
}

impl From<Body> for hyper::Body {
    #[inline]
    fn from(body: Body) -> Self {
        match body {
            Body::Empty => hyper::Body::empty(),
            Body::Once(bytes) => hyper::Body::from(bytes),
            Body::Stream(stream) => hyper::Body::wrap_stream(stream),
        }
    }
}

impl Default for Body {
    #[inline]
    fn default() -> Self {
        Self::empty()
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

impl Stream for BodyStream {
    type Item = io::Result<Bytes>;
    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        let counter = self.counter;
        if counter >= self.segments.len() {
            return Poll::Ready(None);
        }
        match futures::ready!(Pin::new(&mut self.segments[counter]).poll_next(cx)) {
            None => {
                self.counter += 1;
                self.poll_next(cx)
            }
            some => Poll::Ready(some),
        }
    }
}

impl Stream for Body {
    type Item = io::Result<Bytes>;
    #[inline]
    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        match &mut *self {
            Body::Empty => Poll::Ready(None),
            Body::Once(bytes) => {
                let data = mem::take(bytes);
                *self = Body::empty();
                Poll::Ready(Some(Ok(data)))
            }
            Body::Stream(stream) => Pin::new(stream).poll_next(cx),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Body;
    use async_std::fs::File;
    use futures::{AsyncReadExt, TryStreamExt};
    use std::io;

    async fn read_body(body: Body) -> io::Result<String> {
        let mut data = String::new();
        body.into_async_read().read_to_string(&mut data).await?;
        Ok(data)
    }

    #[async_std::test]
    async fn body_empty() -> std::io::Result<()> {
        let body = Body::default();
        assert_eq!("", read_body(body).await?);
        Ok(())
    }

    #[async_std::test]
    async fn body_single() -> std::io::Result<()> {
        let mut body = Body::default();
        body.write("Hello, World");
        assert_eq!("Hello, World", read_body(body).await?);
        Ok(())
    }

    #[async_std::test]
    async fn body_multiple() -> std::io::Result<()> {
        let mut body = Body::default();
        body.write("He").write("llo, ").write("World");
        assert_eq!("Hello, World", read_body(body).await?);
        Ok(())
    }

    #[async_std::test]
    async fn body_composed() -> std::io::Result<()> {
        let mut body = Body::stream();
        body.write("He")
            .write("llo, ")
            .write_reader(File::open("../assets/author.txt").await?)
            .write_reader(File::open("../assets/author.txt").await?)
            .write(".");
        assert_eq!("Hello, HexileeHexilee.", read_body(body).await?);
        Ok(())
    }
}
