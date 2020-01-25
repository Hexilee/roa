use futures::io::{BufReader, Cursor};
use futures::task::{Context, Poll};
use futures::AsyncRead as Read;
use http::response::Builder;
use http::StatusCode;
use std::io::Error;
use std::pin::Pin;

pub struct Body {
    counter: usize,
    pub(crate) segments: Vec<Segment>,
}

pub type Segment = Box<dyn Read + Send + Unpin + 'static>;

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

    pub fn status(&mut self, status_code: StatusCode) -> &mut Self {
        self.builder.status(status_code);
        self
    }

    pub fn write(&mut self, reader: impl Read + Send + Unpin + 'static) -> &mut Self {
        self.segments.push(Box::new(reader));
        self
    }

    pub fn write_str(&mut self, data: impl ToString) -> &mut Self {
        self.write(Box::new(Cursor::new(data.to_string().into_bytes())))
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

impl Default for Response {
    fn default() -> Self {
        Self::new()
    }
}

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
        let reader = Pin::new(self_mut.segments[counter].as_mut());
        match reader.poll_read(cx, buf) {
            Poll::Ready(Ok(0)) => {
                self_mut.counter += 1;
                Pin::new(self_mut).poll_read(cx, buf)
            }
            ret => ret,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Body, Response};
    use async_std::fs::File;
    use futures::AsyncReadExt;

    #[async_std::test]
    async fn body_empty() -> std::io::Result<()> {
        let resp = Response::new();
        let mut data = String::new();
        Body::new(resp.segments).read_to_string(&mut data).await?;
        assert_eq!("", data);
        Ok(())
    }

    #[async_std::test]
    async fn body_single() -> std::io::Result<()> {
        let mut resp = Response::new();
        let mut data = String::new();
        resp.write(b"Hello, World".as_ref());
        Body::new(resp.segments).read_to_string(&mut data).await?;
        assert_eq!("Hello, World", data);
        Ok(())
    }

    #[async_std::test]
    async fn body_multiple() -> std::io::Result<()> {
        let mut resp = Response::new();
        resp.write(b"He".as_ref())
            .write(b"llo, ".as_ref())
            .write(b"World".as_ref());
        let mut data = String::new();
        Body::new(resp.segments).read_to_string(&mut data).await?;
        assert_eq!("Hello, World", data);
        Ok(())
    }

    #[async_std::test]
    async fn body_composed() -> std::io::Result<()> {
        let mut resp = Response::new();
        resp.write(b"He".as_ref())
            .write(b"llo, ".as_ref())
            .write(File::open("assets/test_data.txt").await?)
            .write(b".".as_ref());
        let mut data = String::new();
        Body::new(resp.segments).read_to_string(&mut data).await?;
        assert_eq!("Hello, Hexilee.", data);
        Ok(())
    }

    #[async_std::test]
    async fn response_write_str() -> std::io::Result<()> {
        let mut resp = Response::new();
        let mut data = String::new();
        resp.write_str("Hello, World");
        Body::new(resp.segments).read_to_string(&mut data).await?;
        assert_eq!("Hello, World", data);
        Ok(())
    }
}
