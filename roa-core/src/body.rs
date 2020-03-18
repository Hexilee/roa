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
///
/// async fn read_body(body: Body) -> io::Result<Vec<u8>> {
///     Ok(match body {
///         Body::Bytes(bytes) => bytes.bytes().to_vec(),
///         Body::Stream(mut stream) => {
///             let mut bytes = Vec::new();
///             while let Some(item) = stream.next().await {
///                 bytes.extend_from_slice(&*item?);
///             }
///             bytes
///         }
///     })
/// }
/// ```
pub enum Body {
    /// Bytes kind.
    Bytes(BodyBytes),

    /// Stream kind.
    Stream(BodyStream),
}

/// Bytes based body.
#[derive(Default)]
pub struct BodyBytes {
    size_hint: usize,
    data: Vec<Bytes>,
}

/// Stream based body.
#[derive(Default)]
pub struct BodyStream {
    counter: usize,
    segments: Vec<Segment>,
}

type Segment = Box<dyn Stream<Item = io::Result<Bytes>> + Sync + Send + Unpin + 'static>;

impl Body {
    /// Construct an empty body of bytes kind.
    #[inline]
    pub fn bytes() -> Self {
        Body::Bytes(BodyBytes {
            size_hint: 0,
            data: Vec::new(),
        })
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
            Body::Stream(body_stream) => {
                body_stream.write_stream(stream);
                self
            }
            Body::Bytes(bytes) => {
                let data = mem::take(bytes).bytes();
                *self = Self::stream();
                if !data.is_empty() {
                    self.write(data);
                }
                self.write_stream(stream)
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
    #[inline]
    pub fn write(&mut self, data: impl Into<Bytes>) -> &mut Self {
        match self {
            Body::Bytes(bytes) => bytes.write(data),
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

impl BodyBytes {
    #[inline]
    fn write(&mut self, bytes: impl Into<Bytes>) {
        let data = bytes.into();
        self.size_hint += data.len();
        self.data.push(data);
    }

    /// Consume self and return a bytes.
    #[inline]
    pub fn bytes(mut self) -> Bytes {
        match self.data.len() {
            0 => Bytes::new(),
            1 => self.data.remove(0),
            _ => {
                let mut bytes = BytesMut::with_capacity(self.size_hint);
                for data in self.data.iter() {
                    bytes.extend_from_slice(data)
                }
                bytes.freeze()
            }
        }
    }

    /// Get size hint.
    #[inline]
    pub fn size_hint(&self) -> usize {
        self.size_hint
    }
}

impl From<Body> for hyper::Body {
    #[inline]
    fn from(body: Body) -> Self {
        match body {
            Body::Bytes(bytes) => {
                let data = bytes.bytes();
                if data.is_empty() {
                    hyper::Body::empty()
                } else {
                    hyper::Body::from(data)
                }
            }
            Body::Stream(stream) => hyper::Body::wrap_stream(stream),
        }
    }
}

impl Default for Body {
    #[inline]
    fn default() -> Self {
        Self::bytes()
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
            Body::Bytes(bytes) => {
                if bytes.size_hint == 0 {
                    Poll::Ready(None)
                } else {
                    Poll::Ready(Some(Ok(mem::take(bytes).bytes())))
                }
            }
            Body::Stream(stream) => Pin::new(stream).poll_next(cx),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Body;
    use futures::{AsyncReadExt, TryStreamExt};
    use std::io;
    use tokio::fs::File;

    async fn read_body(body: Body) -> io::Result<String> {
        let mut data = String::new();
        body.into_async_read().read_to_string(&mut data).await?;
        Ok(data)
    }

    #[tokio::test]
    async fn body_empty() -> std::io::Result<()> {
        let body = Body::default();
        assert_eq!("", read_body(body).await?);
        Ok(())
    }

    #[tokio::test]
    async fn body_single() -> std::io::Result<()> {
        let mut body = Body::default();
        body.write("Hello, World");
        assert_eq!("Hello, World", read_body(body).await?);
        Ok(())
    }

    #[tokio::test]
    async fn body_multiple() -> std::io::Result<()> {
        let mut body = Body::default();
        body.write("He").write("llo, ").write("World");
        assert_eq!("Hello, World", read_body(body).await?);
        Ok(())
    }

    #[tokio::test]
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
