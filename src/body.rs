//! The body module of roa.
//! This module provides a context extension `PowerBody`.
//!
//! ### Read/write body in a simpler way.
//!
//! The `roa_core` provides several methods to read/write body.
//!
//! ```rust
//! use roa::core::{Context, Result};
//! use futures::AsyncReadExt;
//! use futures::io::BufReader;
//! use async_std::fs::File;
//!
//! async fn get(mut ctx: Context<()>) -> Result {
//!     // roa_core::Body implements futures::AsyncBufRead.
//!     let mut data = String::new();
//!     ctx.req_mut().await.read_to_string(&mut data).await?;
//!     println!("data: {}", data);
//!
//!     ctx.resp_mut()
//!        .await
//!        // write object implementing futures::AsyncRead
//!        .write(File::open("assets/author.txt").await?)
//!        // write object implementing futures::AsyncBufRead
//!        .write_buf(BufReader::new(File::open("assets/author.txt").await?))
//!        .write_buf(b"Hello, World!".as_ref())
//!        // write `impl ToString`
//!        .write_str("I am Roa.")
//!        // write `impl Into<Vec<u8>>`
//!        .write_bytes(b"Hey Roa.".as_ref());
//!     Ok(())
//! }
//! ```
//!
//! These methods are useful, but they do not deal with headers, especially `Content-*` headers.
//!
//! The `PowerBody` provides more powerful methods to handle it.
//!
//! ```rust
//! use roa::core::{Context, Result};
//! use roa::body::PowerBody;
//! use serde::{Serialize, Deserialize};
//! use askama::Template;
//! use async_std::fs::File;
//! use futures::io::BufReader;
//!
//! #[derive(Debug, Serialize, Deserialize, Template)]
//! #[template(path = "user.html")]
//! struct User {
//!     id: u64,
//!     name: String,
//! }
//!
//! async fn get(mut ctx: Context<()>) -> Result {
//!     // deserialize User from request automatically by Content-Type.
//!     let mut user: User = ctx.read().await?;
//!
//!     // deserialize as json.
//!     user = ctx.read_json().await?;
//!
//!     // deserialize as x-form-urlencoded.
//!     user = ctx.read_form().await?;
//!
//!     // serialize object and write it to body,
//!     // set "Content-Type"
//!     ctx.write_json(&user).await?;
//!
//!     // open file and write it to body,
//!     // set "Content-Type" and "Content-Disposition"
//!     ctx.write_file("assets/welcome.html").await?;
//!
//!     // write text,
//!     // set "Content-Type"
//!     ctx.write_text("Hello, World!").await?;
//!
//!     // write object implementing AsyncBufRead,
//!     // set "Content-Type"
//!     ctx.write_octet(BufReader::new(File::open("assets/author.txt").await?)).await?;
//!
//!     // render html template, based on [askama](https://github.com/djc/askama).
//!     // set "Content-Type"
//!     ctx.render(&user).await?;
//!     Ok(())
//! }
//! ```

mod decode;
mod json;
mod mime_ext;
mod urlencoded;

use crate::core::{async_trait, throw, Context, Error, Result, State, StatusCode};
use crate::header::FriendlyHeaders;
use askama::Template;
use async_std::fs::File;
use async_std::path::Path;
use futures::{AsyncBufRead as BufRead, AsyncReadExt};
use mime::Mime;
use mime_ext::MimeExt;
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use serde::de::DeserializeOwned;
use serde::Serialize;

const APPLICATION_JSON_UTF_8: &str = "application/json; charset=utf-8";

/// A context extension to read/write body more simply.
#[async_trait]
pub trait PowerBody {
    /// try to get mime content type of request.
    async fn request_type(&self) -> Option<Result<Mime>>;

    /// try to get mime content type of response.
    async fn response_type(&self) -> Option<Result<Mime>>;

    /// read request body as Vec<u8>.
    async fn body_buf(&mut self) -> Result<Vec<u8>>;

    /// read request body by Content-Type.
    async fn read<B: DeserializeOwned>(&mut self) -> Result<B>;

    /// read request body as "application/json".
    async fn read_json<B: DeserializeOwned>(&mut self) -> Result<B>;

    /// read request body as "application/x-www-form-urlencoded".
    async fn read_form<B: DeserializeOwned>(&mut self) -> Result<B>;

    // read request body as "multipart/form-data"
    // async fn read_multipart(&self) -> Result<B, Status>;

    /// write object to response body as "application/json; charset=utf-8"
    async fn write_json<B: Serialize + Sync>(&mut self, data: &B) -> Result;

    /// write object to response body as "text/html; charset=utf-8"
    async fn render<B: Template + Sync>(&mut self, data: &B) -> Result;

    /// write object to response body as "text/plain; charset=utf-8"
    async fn write_text<S: ToString + Send>(&mut self, string: S) -> Result;

    /// write object to response body as "application/octet-stream"
    async fn write_octet<B: 'static + BufRead + Unpin + Sync + Send>(
        &mut self,
        reader: B,
    ) -> Result;

    /// write object to response body as extension name of file
    async fn write_file<P: AsRef<Path> + Send>(&mut self, path: P) -> Result;
}

fn parse_mime(value: &str) -> Result<Mime> {
    value.parse().map_err(|err| {
        Error::new(
            StatusCode::BAD_REQUEST,
            format!("{}\nContent-Type value is invalid", err),
            true,
        )
    })
}

#[async_trait]
impl<S: State> PowerBody for Context<S> {
    async fn request_type(&self) -> Option<Result<Mime>> {
        self.req()
            .await
            .get(http::header::CONTENT_TYPE)
            .map(|result| result.and_then(parse_mime))
    }

    async fn response_type(&self) -> Option<Result<Mime>> {
        self.resp()
            .await
            .get(http::header::CONTENT_TYPE)
            .map(|result| result.and_then(parse_mime))
    }

    async fn body_buf(&mut self) -> Result<Vec<u8>> {
        let mut data = Vec::new();
        self.req_mut().await.read_to_end(&mut data).await?;
        Ok(data)
    }

    // return BAD_REQUEST status while parsing Content-Type fails.
    // Content-Type can only be JSON or URLENCODED, otherwise this function will return UNSUPPORTED_MEDIA_TYPE error.
    async fn read<B: DeserializeOwned>(&mut self) -> Result<B> {
        match self.request_type().await {
            None => self.read_json().await,
            Some(ret) => {
                let mime_type = ret?.pure_type();
                if mime_type == mime::APPLICATION_JSON {
                    self.read_json().await
                } else if mime_type == mime::APPLICATION_WWW_FORM_URLENCODED {
                    self.read_form().await
                } else {
                    throw!(
                        StatusCode::UNSUPPORTED_MEDIA_TYPE,
                        "Content-Type can only be JSON or URLENCODED"
                    )
                }
            }
        }
    }

    async fn read_json<B: DeserializeOwned>(&mut self) -> Result<B> {
        let data = self.body_buf().await?;
        match self.request_type().await {
            None | Some(Err(_)) => json::from_bytes(&data),
            Some(Ok(mime_type)) => {
                if mime_type.pure_type() != mime::APPLICATION_JSON {
                    json::from_bytes(&data)
                } else {
                    match mime_type.get_param("charset") {
                        None | Some(mime::UTF_8) => json::from_bytes(&data),
                        Some(charset) => {
                            json::from_str(&decode::decode(&data, charset.as_str())?)
                        }
                    }
                }
            }
        }
    }

    async fn read_form<B: DeserializeOwned>(&mut self) -> Result<B> {
        urlencoded::from_bytes(&self.body_buf().await?)
    }

    async fn write_json<B: Serialize + Sync>(&mut self, data: &B) -> Result {
        self.resp_mut().await.write_bytes(json::to_bytes(data)?);
        self.resp_mut()
            .await
            .insert(http::header::CONTENT_TYPE, APPLICATION_JSON_UTF_8)?;
        Ok(())
    }

    async fn render<B: Template + Sync>(&mut self, data: &B) -> Result {
        self.resp_mut().await.write_str(
            data.render().map_err(|err| {
                Error::new(StatusCode::INTERNAL_SERVER_ERROR, err, false)
            })?,
        );
        self.resp_mut()
            .await
            .insert(http::header::CONTENT_TYPE, &mime::TEXT_HTML_UTF_8)?;
        Ok(())
    }

    async fn write_text<Str: ToString + Send>(&mut self, string: Str) -> Result {
        self.resp_mut().await.write_str(string.to_string());
        self.resp_mut()
            .await
            .insert(http::header::CONTENT_TYPE, &mime::TEXT_PLAIN_UTF_8)?;
        Ok(())
    }

    async fn write_octet<B: 'static + BufRead + Unpin + Sync + Send>(
        &mut self,
        reader: B,
    ) -> Result {
        self.resp_mut().await.write_buf(reader);
        self.resp_mut()
            .await
            .insert(http::header::CONTENT_TYPE, &mime::APPLICATION_OCTET_STREAM)?;
        Ok(())
    }

    async fn write_file<P: AsRef<Path> + Send>(&mut self, path: P) -> Result {
        let path = path.as_ref();
        self.resp_mut().await.write(File::open(path).await?);

        if let Some(filename) = path.file_name() {
            self.resp_mut().await.insert(
                http::header::CONTENT_TYPE,
                &mime_guess::from_path(&filename).first_or_octet_stream(),
            )?;
            let encoded_filename =
                utf8_percent_encode(&filename.to_string_lossy(), NON_ALPHANUMERIC)
                    .to_string();
            self.resp_mut().await.insert(
                http::header::CONTENT_DISPOSITION,
                &format!(
                    "filename={}; filename*=utf-8''{}",
                    &encoded_filename, &encoded_filename
                ),
            )?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{PowerBody, APPLICATION_JSON_UTF_8};
    use crate::core::App;
    use askama::Template;
    use async_std::fs::File;
    use async_std::task::spawn;
    use encoding::EncoderTrap;
    use futures::io::BufReader;
    use http::header::CONTENT_TYPE;
    use http::StatusCode;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize, Hash, Eq, PartialEq, Clone, Template)]
    #[template(path = "user.html")]
    struct User {
        id: u64,
        name: String,
    }

    #[tokio::test]
    async fn read() -> Result<(), Box<dyn std::error::Error>> {
        // miss key
        let (addr, server) = App::new(())
            .end(move |mut ctx| async move {
                let user: User = ctx.read().await?;
                assert_eq!(
                    User {
                        id: 0,
                        name: "Hexilee".to_string()
                    },
                    user
                );
                Ok(())
            })
            .run_local()?;
        spawn(server);
        let client = reqwest::Client::new();

        let data = User {
            id: 0,
            name: "Hexilee".to_string(),
        };
        // err mime type
        let resp = client
            .get(&format!("http://{}", addr))
            .header(CONTENT_TYPE, "text/plain/html")
            .send()
            .await?;
        assert_eq!(StatusCode::BAD_REQUEST, resp.status());
        assert!(resp
            .text()
            .await?
            .ends_with("Content-Type value is invalid"));

        // no Content-Type, default json
        let resp = client
            .get(&format!("http://{}", addr))
            .body(serde_json::to_vec(&data)?)
            .send()
            .await?;
        assert_eq!(StatusCode::OK, resp.status());

        // json
        let resp = client
            .get(&format!("http://{}", addr))
            .json(&data)
            .send()
            .await?;
        assert_eq!(StatusCode::OK, resp.status());

        // x-www-form-urlencoded
        let resp = client
            .get(&format!("http://{}", addr))
            .form(&data)
            .send()
            .await?;
        assert_eq!(StatusCode::OK, resp.status());

        // unsupported Content-Type
        let resp = client
            .get(&format!("http://{}", addr))
            .body(serde_json::to_vec(&data)?)
            .header(CONTENT_TYPE, "text/xml")
            .send()
            .await?;
        assert_eq!(StatusCode::UNSUPPORTED_MEDIA_TYPE, resp.status());
        assert_eq!(
            "Content-Type can only be JSON or URLENCODED",
            resp.text().await?
        );

        // json; encoding
        let resp = client
            .get(&format!("http://{}", addr))
            .body(
                encoding::label::encoding_from_whatwg_label("gbk")
                    .unwrap()
                    .encode(&serde_json::to_string(&data)?, EncoderTrap::Strict)
                    .unwrap(),
            )
            .header(CONTENT_TYPE, "application/json; charset=gbk")
            .send()
            .await?;
        assert_eq!(StatusCode::OK, resp.status());
        Ok(())
    }

    #[tokio::test]
    async fn render() -> Result<(), Box<dyn std::error::Error>> {
        // miss key
        let (addr, server) = App::new(())
            .end(move |mut ctx| async move {
                let user = User {
                    id: 0,
                    name: "Hexilee".to_string(),
                };
                ctx.render(&user).await
            })
            .run_local()?;
        spawn(server);
        let resp = reqwest::get(&format!("http://{}", addr)).await?;
        assert_eq!(StatusCode::OK, resp.status());
        assert_eq!("text/html; charset=utf-8", resp.headers()[CONTENT_TYPE]);
        Ok(())
    }

    #[tokio::test]
    async fn write_text() -> Result<(), Box<dyn std::error::Error>> {
        // miss key
        let (addr, server) = App::new(())
            .end(move |mut ctx| async move { ctx.write_text("Hello, World!").await })
            .run_local()?;
        spawn(server);
        let resp = reqwest::get(&format!("http://{}", addr)).await?;
        assert_eq!(StatusCode::OK, resp.status());
        assert_eq!("text/plain; charset=utf-8", resp.headers()[CONTENT_TYPE]);
        assert_eq!("Hello, World!", resp.text().await?);
        Ok(())
    }

    #[tokio::test]
    async fn write_octet() -> Result<(), Box<dyn std::error::Error>> {
        // miss key
        let (addr, server) = App::new(())
            .end(move |mut ctx| async move {
                ctx.write_octet(BufReader::new(File::open("assets/author.txt").await?))
                    .await
            })
            .run_local()?;
        spawn(server);
        let resp = reqwest::get(&format!("http://{}", addr)).await?;
        assert_eq!(StatusCode::OK, resp.status());
        assert_eq!(
            mime::APPLICATION_OCTET_STREAM.as_ref(),
            resp.headers()[CONTENT_TYPE]
        );
        assert_eq!("Hexilee", resp.text().await?);
        Ok(())
    }

    #[tokio::test]
    async fn response_type() -> Result<(), Box<dyn std::error::Error>> {
        // miss key
        let (addr, server) = App::new(())
            .end(move |mut ctx| async move {
                ctx.write_json(&()).await?;
                assert_eq!(
                    APPLICATION_JSON_UTF_8,
                    ctx.response_type().await.unwrap().unwrap().as_ref()
                );
                Ok(())
            })
            .run_local()?;
        spawn(server);
        let resp = reqwest::get(&format!("http://{}", addr)).await?;
        assert_eq!(StatusCode::OK, resp.status());
        assert_eq!(APPLICATION_JSON_UTF_8, resp.headers()[CONTENT_TYPE]);
        Ok(())
    }
}
