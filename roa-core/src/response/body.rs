use async_std::io::{BufRead, BufReader, Cursor, Error, Read};
use async_std::stream::Stream;
use async_std::task::{Context, Poll};
use std::pin::Pin;

/// Callback when body is finished.
type Callback = dyn 'static + Sync + Send + Unpin + Fn(&Body);

/// The Body of Request and Response.
/// ### Example
/// ```rust
/// use roa_core::Body;
/// use async_std::fs::File;
/// use futures::io::AsyncReadExt;
///
/// #[async_std::main]
/// async fn main() -> std::io::Result<()> {
///     let mut body = Body::default();
///     let mut data = String::new();
///     body.write_buf(b"He".as_ref())
///         .write_buf(b"llo, ".as_ref())
///         .write(File::open("../assets/author.txt").await?)
///         .write_buf(b".".as_ref())
///         .read_to_string(&mut data)
///         .await?;
///     assert_eq!("Hello, Hexilee.", data);
///     Ok(())
/// }
/// ```
pub struct Body {
    counter: usize,
    segments: Vec<Segment>,
    consumed: usize,
    finish: Vec<Box<Callback>>,
}

pub type Segment = Box<dyn BufRead + Sync + Send + Unpin + 'static>;

impl Body {
    pub(crate) fn new() -> Self {
        Self {
            counter: 0,
            segments: Vec::new(),
            consumed: 0,
            finish: Vec::new(),
        }
    }

    /// Write reader implementing BufRead.
    #[inline]
    pub fn write_buf(
        &mut self,
        buf_reader: impl BufRead + Sync + Send + Unpin + 'static,
    ) -> &mut Self {
        self.segments.push(Box::new(buf_reader));
        self
    }

    /// Write reader implementing Read.
    #[inline]
    pub fn write(
        &mut self,
        reader: impl Read + Sync + Send + Unpin + 'static,
    ) -> &mut Self {
        self.write_buf(BufReader::new(reader))
    }

    /// Write `Vec<u8>`.
    #[inline]
    pub fn write_bytes(&mut self, bytes: impl Into<Vec<u8>>) -> &mut Self {
        self.write_buf(Cursor::new(bytes.into()))
    }

    /// Write `String`.
    #[inline]
    pub fn write_str(&mut self, data: impl ToString) -> &mut Self {
        self.write_bytes(data.to_string())
    }

    /// Into a stream.
    #[inline]
    pub fn stream(self) -> BodyStream<Self> {
        BodyStream::new(self)
    }

    #[inline]
    fn poll_segment(&mut self, cx: &mut Context<'_>) -> Poll<Result<&[u8], Error>> {
        Pin::new(self.segments[self.counter].as_mut()).poll_fill_buf(cx)
    }

    /// Callback when dropping body.
    /// ### Example
    /// ```rust
    /// use roa_core::response::Body;
    /// use async_std::fs::File;
    /// use futures::io::AsyncReadExt;
    /// #[async_std::test]
    /// async fn body_on_finish() -> std::io::Result<()> {
    ///     let mut body = Body::default();
    ///     let mut data = String::new();
    ///     body.write_buf(b"He".as_ref())
    ///         .write_buf(b"llo, ".as_ref())
    ///         .write_buf(b"World".as_ref())
    ///         .on_finish(|body| assert_eq!(12, body.consumed()))
    ///         .read_to_string(&mut data)
    ///         .await?;
    ///     assert_eq!("Hello, World", data);
    ///     Ok(())
    /// }
    /// ```
    #[inline]
    pub fn on_finish(
        &mut self,
        callback: impl 'static + Sync + Send + Unpin + Fn(&Self),
    ) -> &mut Self {
        self.finish.push(Box::new(callback));
        self
    }

    /// Get the numbers of consumed bytes.
    #[inline]
    pub fn consumed(&self) -> usize {
        self.consumed
    }
}

impl Default for Body {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Body {
    fn drop(&mut self) {
        let immut_ref = &*self;
        for callback in immut_ref.finish.iter() {
            callback(immut_ref)
        }
    }
}

impl BufRead for Body {
    fn poll_fill_buf(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<&[u8], Error>> {
        let mut_ref = self.get_mut();
        let data = loop {
            if mut_ref.counter >= mut_ref.segments.len() {
                break b"".as_ref();
            }
            let buf = futures::ready!(mut_ref.poll_segment(cx))?;
            let buf_ptr = buf as *const [u8];
            if !buf.is_empty() {
                break unsafe { &*buf_ptr };
            }
            mut_ref.counter += 1;
        };
        Poll::Ready(Ok(data))
    }

    #[inline]
    fn consume(self: Pin<&mut Self>, amt: usize) {
        let self_mut = self.get_mut();
        if self_mut.counter < self_mut.segments.len() {
            Pin::new(self_mut.segments[self_mut.counter].as_mut()).consume(amt);
            self_mut.consumed += amt;
        }
    }
}

impl Read for Body {
    #[inline]
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
    #[inline]
    pub fn new(body: R) -> Self {
        Self { body }
    }
}

impl<R: BufRead + Unpin> Stream for BodyStream<R> {
    type Item = Result<Vec<u8>, std::io::Error>;

    #[inline]
    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        let buf: &[u8] = futures::ready!(Pin::new(&mut self.body).poll_fill_buf(cx))?;
        let buf_len = buf.len();
        if buf_len == 0 {
            Poll::Ready(None)
        } else {
            let data = buf.to_vec();
            Pin::new(&mut self.body).consume(buf_len);
            Poll::Ready(Some(Ok(data)))
        }
    }
}

impl<R: BufRead + Unpin + Send + Sync + 'static> From<BodyStream<R>> for hyper::Body {
    #[inline]
    fn from(stream: BodyStream<R>) -> Self {
        hyper::Body::wrap_stream(stream)
    }
}

#[cfg(test)]
mod tests {
    use super::Body;
    use async_std::fs::File;
    use async_std::io::ReadExt;

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
            .write(File::open("../assets/author.txt").await?)
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

    #[async_std::test]
    async fn body_on_finish() -> std::io::Result<()> {
        let mut body = Body::new();
        let mut data = String::new();
        body.write_buf(b"He".as_ref())
            .write_buf(b"llo, ".as_ref())
            .write_buf(b"World".as_ref())
            .on_finish(|body| assert_eq!(12, body.consumed()))
            .read_to_string(&mut data)
            .await?;
        assert_eq!("Hello, World", data);
        Ok(())
    }
}
