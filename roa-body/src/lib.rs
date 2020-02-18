//! The body crate of roa.
//! This module provides a context extension `PowerBody`.
//!
//! ### Read/write body in a simpler way.
//!
//! The `roa_core` provides several methods to read/write body.
//!
//! ```rust
//! use roa_core::{Context, Result};
//! use futures::AsyncReadExt;
//! use futures::io::BufReader;
//! use async_std::fs::File;
//!
//! async fn get(mut ctx: Context<()>) -> Result {
//!     // roa_core::Body implements futures::AsyncBufRead.
//!     let mut data = String::new();
//!     ctx.req_mut().read_to_string(&mut data).await?;
//!     println!("data: {}", data);
//!
//!     ctx.resp_mut()
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
//! use roa_core::{Context, Result};
//! use roa_body::PowerBody;
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
//!     // deserialize as json.
//!     let mut user: User = ctx.read_json().await?;
//!
//!     // deserialize as x-form-urlencoded.
//!     user = ctx.read_form().await?;
//!
//!     // read multipart form
//!     let form = ctx.multipart();
//!
//!     // serialize object and write it to body,
//!     // set "Content-Type"
//!     ctx.write_json(&user)?;
//!
//!     // open file and write it to body,
//!     // set "Content-Type" and "Content-Disposition"
//!     ctx.write_file("assets/welcome.html").await?;
//!
//!     // write text,
//!     // set "Content-Type"
//!     ctx.write_text("Hello, World!")?;
//!
//!     // write object implementing AsyncBufRead,
//!     // set "Content-Type"
//!     ctx.write_octet(BufReader::new(File::open("assets/author.txt").await?))?;
//!
//!     // render html template, based on [askama](https://github.com/djc/askama).
//!     // set "Content-Type"
//!     ctx.render(&user)?;
//!     Ok(())
//! }
//! ```

mod content_type;
mod help;
use content_type::{Content, ContentType};
use futures::{AsyncBufRead as BufRead, AsyncReadExt};
use help::bug_report;
use roa_core::{async_trait, Context, Result, State};

#[cfg(feature = "json")]
mod decode;
#[cfg(feature = "json")]
mod json;
#[cfg(feature = "multipart")]
mod multipart;
#[cfg(feature = "multipart")]
use multipart::Multipart;
#[cfg(feature = "urlencoded")]
mod urlencoded;
#[cfg(feature = "template")]
use askama::Template;
#[cfg(feature = "file")]
mod file;
#[cfg(feature = "file")]
use file::{write_file, Path};
#[cfg(any(feature = "json", feature = "urlencoded"))]
use serde::de::DeserializeOwned;

#[cfg(feature = "json")]
use serde::Serialize;

/// A context extension to read/write body more simply.
#[async_trait(?Send)]
pub trait PowerBody: Content {
    /// read request body as Vec<u8>.
    async fn body_buf(&mut self) -> Result<Vec<u8>>;

    /// read request body as "application/json".
    #[cfg(feature = "json")]
    async fn read_json<B: DeserializeOwned>(&mut self) -> Result<B>;

    /// read request body as "application/x-www-form-urlencoded".
    #[cfg(feature = "urlencoded")]
    async fn read_form<B: DeserializeOwned>(&mut self) -> Result<B>;

    // read request body as "multipart/form-data"
    #[cfg(feature = "multipart")]
    fn multipart(&mut self) -> Multipart;

    /// write object to response body as "application/json; charset=utf-8"
    #[cfg(feature = "json")]
    fn write_json<B: Serialize + Sync>(&mut self, data: &B) -> Result;

    /// write object to response body as "text/html; charset=utf-8"
    #[cfg(feature = "template")]
    fn render<B: Template + Sync>(&mut self, data: &B) -> Result;

    /// write object to response body as "text/plain; charset=utf-8"
    fn write_text<S: ToString + Send>(&mut self, string: S) -> Result;

    /// write object to response body as "application/octet-stream"
    fn write_octet<B: 'static + BufRead + Unpin + Sync + Send>(
        &mut self,
        reader: B,
    ) -> Result;

    /// write object to response body as extension name of file
    #[cfg(feature = "file")]
    async fn write_file<P: 'static + AsRef<Path> + Send>(&mut self, path: P) -> Result;
}

#[async_trait(?Send)]
impl<S: State> PowerBody for Context<S> {
    async fn body_buf(&mut self) -> Result<Vec<u8>> {
        let mut data = Vec::new();
        self.req_mut().read_to_end(&mut data).await?;
        Ok(data)
    }

    #[cfg(feature = "json")]
    async fn read_json<B: DeserializeOwned>(&mut self) -> Result<B> {
        let content_type = self.content_type()?;
        content_type.expect(mime::APPLICATION_JSON)?;
        let data = self.body_buf().await?;
        match content_type.charset() {
            None | Some(mime::UTF_8) => json::from_bytes(&data),
            Some(charset) => json::from_str(&decode::decode(&data, charset.as_str())?),
        }
    }

    #[cfg(feature = "urlencoded")]
    async fn read_form<B: DeserializeOwned>(&mut self) -> Result<B> {
        self.content_type()?
            .expect(mime::APPLICATION_WWW_FORM_URLENCODED)?;
        urlencoded::from_bytes(&self.body_buf().await?)
    }

    #[cfg(feature = "multipart")]
    fn multipart(&mut self) -> Multipart {
        Multipart::new(self)
    }

    #[cfg(feature = "json")]
    fn write_json<B: Serialize + Sync>(&mut self, data: &B) -> Result {
        self.resp_mut().write_bytes(json::to_bytes(data)?);
        let content_type: ContentType = "application/json; charset=utf-8".parse()?;
        self.resp_mut()
            .headers
            .insert(http::header::CONTENT_TYPE, content_type.to_value()?);
        Ok(())
    }

    #[cfg(feature = "template")]
    fn render<B: Template + Sync>(&mut self, data: &B) -> Result {
        self.resp_mut()
            .write_str(data.render().map_err(bug_report)?);
        let content_type: ContentType = "text/html; charset=utf-8".parse()?;
        self.resp_mut()
            .headers
            .insert(http::header::CONTENT_TYPE, content_type.to_value()?);
        Ok(())
    }

    fn write_text<Str: ToString + Send>(&mut self, string: Str) -> Result {
        self.resp_mut().write_str(string.to_string());
        let content_type: ContentType = "text/plain; charset=utf-8".parse()?;
        self.resp_mut()
            .headers
            .insert(http::header::CONTENT_TYPE, content_type.to_value()?);
        Ok(())
    }

    fn write_octet<B: 'static + BufRead + Unpin + Sync + Send>(
        &mut self,
        reader: B,
    ) -> Result {
        self.resp_mut().write_buf(reader);
        let content_type: ContentType = "application/octet-stream".parse()?;
        self.resp_mut()
            .headers
            .insert(http::header::CONTENT_TYPE, content_type.to_value()?);
        Ok(())
    }

    #[cfg(feature = "file")]
    async fn write_file<P: 'static + AsRef<Path> + Send>(&mut self, path: P) -> Result {
        write_file(self, path).await
    }
}

#[cfg(test)]
mod tests {
    use super::PowerBody;
    use askama::Template;
    use async_std::fs::File;
    use async_std::task::spawn;
    use encoding::EncoderTrap;
    use futures::io::BufReader;
    use http::header::CONTENT_TYPE;
    use http::StatusCode;
    use roa_core::App;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize, Hash, Eq, PartialEq, Clone, Template)]
    #[template(path = "user.html")]
    struct User {
        id: u64,
        name: String,
    }

    #[tokio::test]
    async fn read_json() -> Result<(), Box<dyn std::error::Error>> {
        // miss key
        let (addr, server) = App::new(())
            .end(move |mut ctx| async move {
                let user: User = ctx.read_json().await?;
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

        // json
        let resp = client
            .get(&format!("http://{}", addr))
            .json(&data)
            .send()
            .await?;
        assert_eq!(StatusCode::OK, resp.status());

        // unmatched Content-Type
        let resp = client
            .get(&format!("http://{}", addr))
            .body(serde_json::to_vec(&data)?)
            .header(CONTENT_TYPE, "text/xml")
            .send()
            .await?;
        assert_eq!(StatusCode::BAD_REQUEST, resp.status());
        assert_eq!(
            "content type unmatched. application/json != text/xml",
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
    async fn read_form() -> Result<(), Box<dyn std::error::Error>> {
        // miss key
        let (addr, server) = App::new(())
            .end(move |mut ctx| async move {
                let user: User = ctx.read_form().await?;
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

        // x-www-form-urlencoded
        let resp = client
            .get(&format!("http://{}", addr))
            .form(&data)
            .send()
            .await?;
        assert_eq!(StatusCode::OK, resp.status());

        // unmatched Content-Type
        let resp = client
            .get(&format!("http://{}", addr))
            .body(serde_json::to_vec(&data)?)
            .header(CONTENT_TYPE, "text/xml")
            .send()
            .await?;
        assert_eq!(StatusCode::BAD_REQUEST, resp.status());
        assert_eq!(
            "content type unmatched. application/x-www-form-urlencoded != text/xml",
            resp.text().await?
        );

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
                ctx.render(&user)
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
            .end(move |mut ctx| async move { ctx.write_text("Hello, World!") })
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
                ctx.write_octet(BufReader::new(
                    File::open("../assets/author.txt").await?,
                ))
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
}
