use bytes::{Buf, Bytes, BytesMut};
use futures::future::{ok, Ready};
use futures::io::{self, AsyncRead};
use futures::stream::{once, Once, Stream};
use std::mem;
use std::pin::Pin;
use std::task::{Context, Poll};

const DEFAULT_CHUNK_SIZE: usize = 4096;

/// The body of response.
pub enum Body {
    Bytes {
        size_hint: usize,
        data: Vec<Bytes>,
    },
    Stream {
        counter: usize,
        segments: Vec<Segment>,
    },
}

type Segment = Box<dyn Stream<Item = io::Result<Bytes>> + Sync + Send + Unpin + 'static>;

impl Body {
    #[inline]
    pub fn bytes() -> Self {
        Body::Bytes {
            size_hint: 0,
            data: Vec::new(),
        }
    }

    #[inline]
    pub fn stream() -> Self {
        Body::Stream {
            counter: 0,
            segments: Vec::new(),
        }
    }

    /// Write stream.
    pub fn write_stream(
        &mut self,
        stream: impl Stream<Item = io::Result<Bytes>> + Sync + Send + Unpin + 'static,
    ) -> &mut Self {
        match self {
            Body::Stream { counter, segments } => {
                segments.push(Box::new(stream));
                self
            }
            Body::Bytes { size_hint, data } => {
                let size = *size_hint;
                let data = mem::take(data);
                *self = Self::stream();
                if !data.is_empty() {
                    self.write_stream(bytes_stream(join_bytes(size, data)));
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
    pub fn write(&mut self, into_bytes: impl Into<Bytes>) -> &mut Self {
        let bytes = into_bytes.into();
        match self {
            Body::Bytes { size_hint, data } => {
                *size_hint += bytes.len();
                data.push(bytes);
                self
            }
            stream @ Body::Stream { .. } => stream.write_stream(bytes_stream(bytes)),
        }
    }

    /// Wrap self with a wrapper.
    #[inline]
    pub fn wrapped(&mut self, wrapper: impl FnOnce(Self) -> Self) -> &mut Self {
        *self = wrapper(std::mem::take(self));
        self
    }
}

impl From<Body> for hyper::Body {
    #[inline]
    fn from(body: Body) -> Self {
        match body {
            Body::Bytes { size_hint, data } => {
                if data.is_empty() {
                    hyper::Body::empty()
                } else {
                    hyper::Body::from(join_bytes(size_hint, data))
                }
            }
            stream @ Body::Stream { .. } => hyper::Body::wrap_stream(stream),
        }
    }
}

impl Default for Body {
    #[inline]
    fn default() -> Self {
        Self::bytes()
    }
}

#[inline]
fn join_bytes(size: usize, mut bytes: Vec<Bytes>) -> Bytes {
    match bytes.len() {
        0 => Bytes::new(),
        1 => bytes.remove(0),
        _ => {
            let mut new_bytes = BytesMut::with_capacity(size);
            for data in bytes.iter() {
                new_bytes.extend_from_slice(data)
            }
            new_bytes.freeze()
        }
    }
}

#[inline]
pub(crate) fn bytes_stream(bytes: Bytes) -> Once<Ready<io::Result<Bytes>>> {
    once(ok(bytes))
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
        match &mut *self {
            Body::Stream { counter, segments } => {
                if *counter >= segments.len() {
                    return Poll::Ready(None);
                }
                match futures::ready!(Pin::new(&mut segments[*counter]).poll_next(cx)) {
                    None => {
                        *counter += 1;
                        self.poll_next(cx)
                    }
                    some => Poll::Ready(some),
                }
            }
            Body::Bytes { size_hint, data } => {
                if data.is_empty() {
                    Poll::Ready(None)
                } else {
                    let data = mem::take(data);
                    Poll::Ready(Some(Ok(join_bytes(*size_hint, data))))
                }
            }
        }
    }
}

//impl Debug for Body {
//    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
//        match self {
//            Body::Bytes { size_hint, data } => f.write_fmt(format_args!(
//                "Body::Bytes: size hint: {}, data: {}",
//                *size_hint,
//                String::from_utf8_lossy(&*join_bytes(*size_hint, data.clone()))
//            )),
//            Body::Stream {}
//        }
//    }
//}

#[cfg(test)]
mod tests {
    use super::Body;
    use async_std::fs::File;
    use futures::AsyncReadExt;
    use futures::TryStreamExt;

    #[async_std::test]
    async fn body_empty() -> std::io::Result<()> {
        let body = Body::default();
        let mut data = String::new();
        body.into_async_read().read_to_string(&mut data).await?;
        assert_eq!("", data);
        Ok(())
    }

    #[async_std::test]
    async fn body_single() -> std::io::Result<()> {
        let mut body = Body::default();
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
        let mut body = Body::default();
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
        let mut body = Body::stream();
        let mut data = String::new();
        body.write("He")
            .write("llo, ")
            .write_reader(File::open("../assets/author.txt").await?)
            .write_reader(File::open("../assets/author.txt").await?)
            .write(".")
            .into_async_read()
            .read_to_string(&mut data)
            .await?;
        assert_eq!("Hello, HexileeHexilee.", data);
        Ok(())
    }
}
