use actix_http::error::PayloadError;
use actix_http::http::HeaderMap;
use actix_multipart::Field as ActixField;
use actix_multipart::Multipart as ActixMultipart;
use actix_multipart::MultipartError;
use bytes::Bytes;
use futures::{AsyncBufRead, Stream};
use roa_core::header::CONTENT_TYPE;
use roa_core::{Context, Error, State, StatusCode};
use std::fmt::{self, Display, Formatter};
use std::io;
use std::ops::Deref;
use std::pin::Pin;
use std::task::{self, Poll};

pub struct Multipart(ActixMultipart);
pub struct Field(ActixField);
#[derive(Debug)]
pub struct WrapError(MultipartError);
struct BodyStream<R: AsyncBufRead>(R);

impl Multipart {
    pub fn new<S: State>(ctx: &mut Context<S>) -> Self {
        let mut map = HeaderMap::new();
        if let Some(value) = ctx.header_value(CONTENT_TYPE) {
            map.insert(CONTENT_TYPE, value)
        }
        let body = std::mem::take(&mut **ctx.req_mut());
        Multipart(ActixMultipart::new(&map, BodyStream(body)))
    }
}

impl<R: AsyncBufRead + Unpin> Stream for BodyStream<R> {
    type Item = Result<Bytes, PayloadError>;

    #[inline]
    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        let buf: &[u8] = futures::ready!(Pin::new(&mut self.0).poll_fill_buf(cx))?;
        let buf_len = buf.len();
        if buf_len == 0 {
            Poll::Ready(None)
        } else {
            let data = Bytes::from(buf.to_vec());
            Pin::new(&mut self.0).consume(buf_len);
            Poll::Ready(Some(Ok(data)))
        }
    }
}

impl Stream for Multipart {
    type Item = Result<Field, WrapError>;

    #[inline]
    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        match Pin::new(&mut self.0).poll_next(cx) {
            Poll::Ready(Some(item)) => Poll::Ready(Some(match item {
                Ok(field) => Ok(Field(field)),
                Err(err) => Err(WrapError(err)),
            })),
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

impl Stream for Field {
    type Item = Result<Bytes, io::Error>;
    #[inline]
    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        match Pin::new(&mut self.0).poll_next(cx) {
            Poll::Ready(Some(item)) => Poll::Ready(Some(match item {
                Ok(bytes) => Ok(bytes),
                Err(err) => Err(match err {
                    MultipartError::Payload(PayloadError::Io(err)) => err,
                    err => io::Error::new(
                        io::ErrorKind::Other,
                        Error::new(
                            StatusCode::INTERNAL_SERVER_ERROR,
                            format!("{}\nread multipart field error.", err),
                            false,
                        ),
                    ),
                }),
            })),
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

impl Deref for Field {
    type Target = ActixField;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<WrapError> for Error {
    fn from(err: WrapError) -> Self {
        Error::new(StatusCode::BAD_REQUEST, err, true)
    }
}

impl Display for WrapError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("{}\nmultipart form read error.", self.0))
    }
}

impl std::error::Error for WrapError {}
