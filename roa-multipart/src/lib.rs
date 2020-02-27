use actix_http::error::PayloadError;
use actix_http::http::HeaderMap;
use actix_multipart::Field as ActixField;
use actix_multipart::Multipart as ActixMultipart;
use actix_multipart::MultipartError;
use bytes::Bytes;
use futures::Stream;
use hyper::Body;
use roa_core::http::{header::CONTENT_TYPE, StatusCode};
use roa_core::{Context, Error, State};
use std::fmt::{self, Display, Formatter};
use std::io;
use std::ops::Deref;
use std::pin::Pin;
use std::task::{self, Poll};

pub struct Multipart(ActixMultipart);
pub struct Field(ActixField);
#[derive(Debug)]
pub struct WrapError(MultipartError);
pub struct WrapStream(Option<Body>);

impl Multipart {
    pub fn new<S: State>(ctx: &mut Context<S>) -> Self {
        let mut map = HeaderMap::new();
        if let Some(value) = ctx.req().headers.get(CONTENT_TYPE) {
            map.insert(CONTENT_TYPE, value.clone())
        }
        Multipart(ActixMultipart::new(
            &map,
            WrapStream(Some(ctx.req_mut().body_stream())),
        ))
    }
}

impl Stream for WrapStream {
    type Item = Result<Bytes, PayloadError>;
    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        match &mut self.0 {
            None => Poll::Ready(None),
            Some(body) => match futures::ready!(Pin::new(body).poll_next(cx)) {
                None => {
                    self.0 = None;
                    self.poll_next(cx)
                }
                Some(item) => Poll::Ready(Some(match item {
                    Ok(data) => Ok(data),
                    Err(err) => Err(if err.is_incomplete_message() {
                        PayloadError::Incomplete(Some(io::Error::new(
                            io::ErrorKind::UnexpectedEof,
                            err,
                        )))
                    } else {
                        PayloadError::Io(io::Error::new(io::ErrorKind::Other, err))
                    }),
                })),
            },
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
        match futures::ready!(Pin::new(&mut self.0).poll_next(cx)) {
            None => Poll::Ready(None),
            Some(item) => Poll::Ready(Some(match item {
                Ok(field) => Ok(Field(field)),
                Err(err) => Err(WrapError(err)),
            })),
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
        match futures::ready!(Pin::new(&mut self.0).poll_next(cx)) {
            None => Poll::Ready(None),
            Some(item) => Poll::Ready(Some(match item {
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
