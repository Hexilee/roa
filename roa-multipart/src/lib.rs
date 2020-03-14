//! This crate provides a wrapper for `actix_multipart::Multipart`,
//! which may cause heavy dependencies.
//!
//! It won't be used as a module of crate `roa` until implementing a cleaner Multipart.  
//!
//! ### Example
//! ```
//! use async_std::fs::File;
//! use async_std::io;
//! use async_std::path::Path;
//! use futures::stream::TryStreamExt;
//! use futures::StreamExt;
//! use roa_core::http::StatusCode;
//! use roa_tcp::Listener;
//! use roa_router::{Router, post};
//! use roa_core::{self as roa, throw, App, Context};
//! use roa_multipart::Multipart;
//! use std::error::Error as StdError;
//!
//! async fn post_file(ctx: &mut Context<()>) -> roa::Result {
//!     let mut form = Multipart::new(ctx);
//!     while let Some(item) = form.next().await {
//!         let field = item?;
//!         match field.content_disposition() {
//!             None => throw!(StatusCode::BAD_REQUEST, "content disposition not set"),
//!             Some(content_disposition) => match content_disposition.get_filename() {
//!                 None => continue, // ignore non-file field
//!                 Some(filename) => {
//!                     let path = Path::new("./upload");
//!                     let mut file = File::create(path.join(filename)).await?;
//!                     io::copy(&mut field.into_async_read(), &mut file).await?;
//!                 }
//!             },
//!         }
//!     }
//!     Ok(())
//! }
//!
//! # fn main() -> Result<(), Box<dyn StdError>> {
//! let router = Router::new().on("file", post(post_file));
//! let (addr, server) = App::new(()).end(router.routes("/")?).run()?;
//! // server.await
//! Ok(())
//! # }
//! ```

#![warn(missing_docs)]

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

/// A wrapper for actix multipart.
pub struct Multipart(ActixMultipart);

/// A wrapper for actix multipart field.
pub struct Field(ActixField);
#[derive(Debug)]

/// A wrapper for actix multipart field.
pub struct WrapError(MultipartError);

/// A wrapper for hyper::Body.
struct WrapStream(Option<Body>);

impl Multipart {
    /// Construct multipart from Context.
    #[inline]
    pub fn new<S: State>(ctx: &mut Context<S>) -> Self {
        let mut map = HeaderMap::new();
        if let Some(value) = ctx.req.headers.get(CONTENT_TYPE) {
            map.insert(CONTENT_TYPE, value.clone())
        }
        Multipart(ActixMultipart::new(
            &map,
            WrapStream(Some(ctx.req.raw_body())),
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
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<WrapError> for Error {
    #[inline]
    fn from(err: WrapError) -> Self {
        Error::new(StatusCode::BAD_REQUEST, err, true)
    }
}

impl Display for WrapError {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("{}\nmultipart form read error.", self.0))
    }
}

impl std::error::Error for WrapError {}

#[cfg(test)]
mod tests {
    use super::Multipart;
    use async_std::fs::{read, read_to_string};
    use futures::stream::TryStreamExt;
    use futures::{AsyncReadExt, StreamExt};
    use reqwest::{
        multipart::{Form, Part},
        Client,
    };
    use roa_core::http::{header::CONTENT_TYPE, StatusCode};
    use roa_core::{self as roa, throw, App, Context};
    use roa_router::{post, Router};
    use roa_tcp::Listener;
    use std::error::Error as StdError;

    const FILE_PATH: &str = "../assets/author.txt";
    const FILE_NAME: &str = "author.txt";
    const FIELD_NAME: &str = "file";

    async fn post_file(ctx: &mut Context<()>) -> roa::Result {
        let mut form = Multipart::new(ctx);
        while let Some(item) = form.next().await {
            let field = item?;
            match field.content_disposition() {
                None => throw!(StatusCode::BAD_REQUEST, "content disposition not set"),
                Some(disposition) => {
                    match (disposition.get_filename(), disposition.get_name()) {
                        (Some(filename), Some(name)) => {
                            assert_eq!(FIELD_NAME, name);
                            assert_eq!(FILE_NAME, filename);
                            let mut content = String::new();
                            field.into_async_read().read_to_string(&mut content).await?;
                            let expected_content = read_to_string(FILE_PATH).await?;
                            assert_eq!(&expected_content, &content);
                        }
                        _ => throw!(StatusCode::BAD_REQUEST, "invalid field"),
                    }
                }
            }
        }
        Ok(())
    }

    #[tokio::test]
    async fn upload() -> Result<(), Box<dyn StdError>> {
        let router = Router::new().on("/file", post(post_file));
        let app = App::new(()).end(router.routes("/")?);
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
