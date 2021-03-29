#![cfg_attr(feature = "docs", feature(external_doc))]
#![cfg_attr(feature = "docs", doc(include = "../README.md"))]
#![cfg_attr(feature = "docs", warn(missing_docs))]

use std::fmt::{self, Display, Formatter};
use std::io;
use std::ops::Deref;
use std::pin::Pin;
use std::task::{self, Poll};

use actix_http::error::PayloadError;
use actix_http::http::HeaderMap;
use actix_multipart::{
    Field as ActixField, Multipart as ActixMultipart, MultipartError as ActixMultipartError,
};
use bytes::Bytes;
use futures::Stream;
use hyper::Body;
use roa_core::http::header::CONTENT_TYPE;
use roa_core::http::StatusCode;
use roa_core::{Context, Status};

/// A context extensio nwrapped `actix_multipart::Multipart`.
pub trait MultipartForm {
    /// Read request body as multipart form.
    fn form(&mut self) -> Multipart;
}

impl<S> MultipartForm for Context<S> {
    fn form(&mut self) -> Multipart {
        let mut map = HeaderMap::new();
        if let Some(value) = self.req.headers.get(CONTENT_TYPE) {
            map.insert(CONTENT_TYPE, value.clone())
        }
        Multipart(ActixMultipart::new(
            &map,
            WrapStream(Some(self.req.raw_body())),
        ))
    }
}

/// A wrapper for actix multipart.
pub struct Multipart(ActixMultipart);

/// A wrapper for actix multipart field.
pub struct Field(ActixField);

/// A wrapper for actix multipart field.
#[derive(Debug)]
pub struct MultipartError(ActixMultipartError);

/// A wrapper for hyper::Body.
struct WrapStream(Option<Body>);

impl Stream for WrapStream {
    type Item = Result<Bytes, PayloadError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Option<Self::Item>> {
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
    type Item = Result<Field, MultipartError>;

    #[inline]
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Option<Self::Item>> {
        match futures::ready!(Pin::new(&mut self.0).poll_next(cx)) {
            None => Poll::Ready(None),
            Some(item) => Poll::Ready(Some(match item {
                Ok(field) => Ok(Field(field)),
                Err(err) => Err(MultipartError(err)),
            })),
        }
    }
}

impl Stream for Field {
    type Item = Result<Bytes, io::Error>;
    #[inline]
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Option<Self::Item>> {
        match futures::ready!(Pin::new(&mut self.0).poll_next(cx)) {
            None => Poll::Ready(None),
            Some(item) => Poll::Ready(Some(match item {
                Ok(bytes) => Ok(bytes),
                Err(err) => Err(match err {
                    ActixMultipartError::Payload(PayloadError::Io(err)) => err,
                    err => io::Error::new(
                        io::ErrorKind::Other,
                        Status::new(
                            StatusCode::INTERNAL_SERVER_ERROR,
                            format!("{}\nread multipart field error.", err),
                            false,
                        )
                        .to_string(),
                    ),
                }),
            })),
        }
    }
}

impl Deref for Field {
    type Target = ActixField;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<MultipartError> for Status {
    #[inline]
    fn from(err: MultipartError) -> Self {
        Status::new(StatusCode::BAD_REQUEST, err, true)
    }
}

impl Display for MultipartError {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("{}\nmultipart form read error.", self.0))
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error as StdError;

    use async_std::fs::{read, read_to_string};
    use futures::stream::TryStreamExt;
    use futures::{AsyncReadExt, StreamExt};
    use reqwest::multipart::{Form, Part};
    use reqwest::Client;
    use roa::http::header::CONTENT_TYPE;
    use roa::http::StatusCode;
    use roa::router::{post, Router};
    use roa::tcp::Listener;
    use roa::{throw, App, Context};

    use super::MultipartForm;

    const FILE_PATH: &str = "../assets/author.txt";
    const FILE_NAME: &str = "author.txt";
    const FIELD_NAME: &str = "file";

    async fn post_file(ctx: &mut Context) -> roa::Result {
        let mut form = ctx.form();
        while let Some(item) = form.next().await {
            let field = item?;
            match field.content_disposition() {
                None => throw!(StatusCode::BAD_REQUEST, "content disposition not set"),
                Some(disposition) => match (disposition.get_filename(), disposition.get_name()) {
                    (Some(filename), Some(name)) => {
                        assert_eq!(FIELD_NAME, name);
                        assert_eq!(FILE_NAME, filename);
                        let mut content = String::new();
                        field.into_async_read().read_to_string(&mut content).await?;
                        let expected_content = read_to_string(FILE_PATH).await?;
                        assert_eq!(&expected_content, &content);
                    }
                    _ => throw!(StatusCode::BAD_REQUEST, "invalid field"),
                },
            }
        }
        Ok(())
    }

    #[tokio::test]
    async fn upload() -> Result<(), Box<dyn StdError>> {
        let router = Router::new().on("/file", post(post_file));
        let app = App::new().end(router.routes("/")?);
        let (addr, server) = app.run()?;
        async_std::task::spawn(server);

        // client
        let url = format!("http://{}/file", addr);
        let client = Client::new();
        let form = Form::new().part(
            FIELD_NAME,
            Part::bytes(read(FILE_PATH).await?).file_name(FILE_NAME),
        );
        let boundary = form.boundary().to_string();
        let resp = client
            .post(&url)
            .body(form.stream())
            .header(
                CONTENT_TYPE,
                format!(r#"multipart/form-data; boundary="{}""#, boundary),
            )
            .send()
            .await?;
        assert_eq!(StatusCode::OK, resp.status());
        Ok(())
    }
}
