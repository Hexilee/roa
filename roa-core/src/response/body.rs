use bytes::{Buf, Bytes, BytesMut};
use futures::io::{AsyncRead, Cursor, Error};
use futures::stream::Stream;
use std::pin::Pin;
use std::task::{Context, Poll};

const CHUNK_SIZE: usize = 1024;

/// The Body of Request and Response.
pub struct Body {
    counter: usize,
    segments: Vec<Segment>,
}

type Segment = Box<dyn AsyncRead + Sync + Send + Unpin + 'static>;

impl Body {
    pub(crate) fn new() -> Self {
        Self {
            counter: 0,
            segments: Vec::new(),
        }
    }

    /// Write reader implementing AsyncRead.
    #[inline]
    pub fn write(
        &mut self,
        buf_reader: impl AsyncRead + Sync + Send + Unpin + 'static,
    ) -> &mut Self {
        self.segments.push(Box::new(buf_reader));
        self
    }

    /// Write `Vec<u8>`.
    #[inline]
    pub fn write_bytes(&mut self, bytes: impl Into<Vec<u8>>) -> &mut Self {
        self.write(Cursor::new(bytes.into()))
    }

    /// Write `String`.
    #[inline]
    pub fn write_str(&mut self, data: impl ToString) -> &mut Self {
        self.write_bytes(data.to_string())
    }

    /// Into a stream.
    #[inline]
    pub fn into_stream(self) -> BodyStream {
        self.into()
    }
}

impl Default for Body {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

pub struct BodyStream(Body);

impl From<Body> for BodyStream {
    fn from(body: Body) -> Self {
        Self(body)
    }
}

impl Stream for BodyStream {
    type Item = Result<Bytes, Error>;
    #[inline]
    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        let mut chunk = BytesMut::with_capacity(CHUNK_SIZE);
        unsafe { chunk.set_len(CHUNK_SIZE) };
        let bytes = futures::ready!(Pin::new(&mut self.0).poll_read(cx, &mut *chunk))?;
        if bytes == 0 {
            Poll::Ready(None)
        } else {
            Poll::Ready(Some(Ok(chunk.to_bytes().slice(0..bytes))))
        }
    }
}

impl AsyncRead for Body {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<usize, Error>> {
        if self.counter >= self.segments.len() {
            return Poll::Ready(Ok(0));
        }
        let counter = self.counter;
        let bytes =
            futures::ready!(Pin::new(&mut self.segments[counter]).poll_read(cx, buf))?;
        if bytes == 0 {
            self.counter += 1;
            return self.poll_read(cx, buf);
        }

        Poll::Ready(Ok(bytes))
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
        body.write(b"Hello, World".as_ref())
            .read_to_string(&mut data)
            .await?;
        assert_eq!("Hello, World", data);
        Ok(())
    }

    #[async_std::test]
    async fn body_multiple() -> std::io::Result<()> {
        let mut body = Body::new();
        let mut data = String::new();
        body.write(b"He".as_ref())
            .write(b"llo, ".as_ref())
            .write(b"World".as_ref())
            .read_to_string(&mut data)
            .await?;
        assert_eq!("Hello, World", data);
        Ok(())
    }

    #[async_std::test]
    async fn body_composed() -> std::io::Result<()> {
        let mut body = Body::new();
        let mut data = String::new();
        body.write(b"He".as_ref())
            .write(b"llo, ".as_ref())
            .write(File::open("../assets/author.txt").await?)
            .write(b".".as_ref())
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

    #[async_std::test]
    async fn stream() -> std::io::Result<()> {
        let mut body = Body::new();
        let mut data = String::new();
        body.write_str("Hello, World");
        body.into_stream()
            .into_async_read()
            .read_to_string(&mut data)
            .await?;
        assert_eq!("Hello, World", data);
        Ok(())
    }
}
