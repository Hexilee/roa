use actix_http::error::PayloadError;
use actix_http::http::header::ContentDisposition;
use actix_http::http::HeaderMap;
use actix_multipart::Field as ActixField;
use actix_multipart::Multipart as ActixMultipart;
use actix_multipart::MultipartError;
use bytes::Bytes;
use futures::lock::Mutex;
use futures::stream::IntoAsyncRead;
use futures::{AsyncBufRead, Stream, TryStreamExt};
use mime::Mime;
use roa_core::header::CONTENT_TYPE;
use roa_core::{Context, Error, State, StatusCode};
use std::fmt::{self, Display, Formatter};
use std::io;
use std::pin::Pin;
use std::task::{self, Poll};

pub struct Multipart(Mutex<ActixMultipart>);
pub struct Field(Mutex<ActixField>);

#[derive(Debug)]
pub struct WrapError(MultipartError);
pub struct BodyStream<R: AsyncBufRead>(R);

impl Multipart {
    pub async fn new<S: State>(ctx: &mut Context<S>) -> Self {
        let mut map = HeaderMap::new();
        if let Some(value) = ctx.header_value(CONTENT_TYPE).await {
            map.insert(CONTENT_TYPE, value)
        }
        let body = std::mem::take(&mut **ctx.req_mut().await);
        Multipart(Mutex::new(ActixMultipart::new(&map, BodyStream(body))))
    }
}

impl Field {
    pub fn reader(self) -> IntoAsyncRead<Self> {
        self.into_async_read()
    }

    pub async fn content_type(&self) -> Mime {
        self.0.lock().await.content_type().clone()
    }

    pub async fn headers(&self) -> HeaderMap {
        self.0.lock().await.headers().clone()
    }

    pub async fn content_disposition(&self) -> Option<ContentDisposition> {
        self.0.lock().await.content_disposition()
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
        self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        match self.0.try_lock() {
            None => Poll::Pending,
            Some(mut form) => match Pin::new(&mut *form).poll_next(cx) {
                Poll::Ready(Some(item)) => Poll::Ready(Some(match item {
                    Ok(field) => Ok(Field(Mutex::new(field))),
                    Err(err) => Err(WrapError(err)),
                })),
                Poll::Ready(None) => Poll::Ready(None),
                Poll::Pending => Poll::Pending,
            },
        }
    }
}

impl Stream for Field {
    type Item = Result<Bytes, io::Error>;
    #[inline]
    fn poll_next(
        self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        match self.0.try_lock() {
            None => Poll::Pending,
            Some(mut field) => match Pin::new(&mut *field).poll_next(cx) {
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
            },
        }
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
