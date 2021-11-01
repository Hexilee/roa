//! This module provides a context extension `PowerBody`.
//!
//! ### Read/write body in a easier way.
//!
//! The `roa_core` provides several methods to read/write body.
//!
//! ```rust
//! use roa::{Context, Result};
//! use futures::AsyncReadExt;
//! use futures::io::BufReader;
//! use async_std::fs::File;
//!
//! async fn get(ctx: &mut Context) -> Result {
//!     let mut data = String::new();
//!     // implements futures::AsyncRead.
//!     ctx.req.reader().read_to_string(&mut data).await?;
//!     println!("data: {}", data);
//!
//!     // although body is empty now...
//!     let stream = ctx.req.stream();
//!     ctx.resp
//!         // echo
//!        .write_stream(stream)
//!        // write object implementing futures::AsyncRead
//!        .write_reader(File::open("assets/author.txt").await?)
//!        // write reader with specific chunk size
//!        .write_chunk(File::open("assets/author.txt").await?, 1024)
//!        // write text
//!        .write("I am Roa.")
//!        .write(b"I am Roa.".as_ref());
//!     Ok(())
//! }
//! ```
//!
//! These methods are useful, but they do not deal with headers and (de)serialization.
//!
//! The `PowerBody` provides more powerful methods to handle it.
//!
//! ```rust
//! use roa::{Context, Result};
//! use roa::body::{PowerBody, DispositionType::*};
//! use serde::{Serialize, Deserialize};
//! use askama::Template;
//! use async_std::fs::File;
//!
//! #[derive(Debug, Serialize, Deserialize, Template)]
//! #[template(path = "user.html")]
//! struct User {
//!     id: u64,
//!     name: String,
//! }
//!
//! async fn get(ctx: &mut Context) -> Result {
//!     // read as bytes.
//!     let data = ctx.read().await?;
//!
//!     // deserialize as json.
//!     let user: User = ctx.read_json().await?;
//!
//!     // deserialize as x-form-urlencoded.
//!     let user: User = ctx.read_form().await?;
//!
//!     // serialize object and write it to body,
//!     // set "Content-Type"
//!     ctx.write_json(&user)?;
//!
//!     // open file and write it to body,
//!     // set "Content-Type" and "Content-Disposition"
//!     ctx.write_file("assets/welcome.html", Inline).await?;
//!
//!     // write text,
//!     // set "Content-Type"
//!     ctx.write("Hello, World!");
//!
//!     // write object implementing AsyncRead,
//!     // set "Content-Type"
//!     ctx.write_reader(File::open("assets/author.txt").await?);
//!
//!     // render html template, based on [askama](https://github.com/djc/askama).
//!     // set "Content-Type"
//!     ctx.render(&user)?;
//!     Ok(())
//! }
//! ```

#[cfg(feature = "template")]
use askama::Template;
use bytes::Bytes;
use futures::{AsyncRead, AsyncReadExt};
use headers::{ContentLength, ContentType, HeaderMapExt};

use crate::{async_trait, http, Context, Result, State};
#[cfg(feature = "file")]
mod file;
#[cfg(feature = "file")]
pub use file::DispositionType;
#[cfg(feature = "file")]
use file::{write_file, Path};
#[cfg(feature = "json")]
pub use multer::Multipart;
#[cfg(any(feature = "json", feature = "urlencoded"))]
use serde::de::DeserializeOwned;
#[cfg(feature = "json")]
use serde::Serialize;

/// A context extension to read/write body more simply.
#[async_trait]
pub trait PowerBody {
    /// read request body as Bytes.
    async fn read(&mut self) -> Result<Vec<u8>>;

    /// read request body as "json".
    #[cfg(feature = "json")]
    #[cfg_attr(feature = "docs", doc(cfg(feature = "json")))]
    async fn read_json<B>(&mut self) -> Result<B>
    where
        B: DeserializeOwned;

    /// read request body as "urlencoded form".
    #[cfg(feature = "urlencoded")]
    #[cfg_attr(feature = "docs", doc(cfg(feature = "urlencoded")))]
    async fn read_form<B>(&mut self) -> Result<B>
    where
        B: DeserializeOwned;

    /// read request body as "multipart form".
    #[cfg(feature = "multipart")]
    #[cfg_attr(feature = "docs", doc(cfg(feature = "multipart")))]
    async fn read_multipart(&mut self) -> Result<Multipart>;

    /// write object to response body as "application/json"
    #[cfg(feature = "json")]
    #[cfg_attr(feature = "docs", doc(cfg(feature = "json")))]
    fn write_json<B>(&mut self, data: &B) -> Result
    where
        B: Serialize;

    /// write object to response body as "text/html; charset=utf-8"
    #[cfg(feature = "template")]
    #[cfg_attr(feature = "docs", doc(cfg(feature = "template")))]
    fn render<B>(&mut self, data: &B) -> Result
    where
        B: Template;

    /// write object to response body as "text/plain"
    fn write<B>(&mut self, data: B)
    where
        B: Into<Bytes>;

    /// write object to response body as "application/octet-stream"
    fn write_reader<B>(&mut self, reader: B)
    where
        B: 'static + AsyncRead + Unpin + Sync + Send;

    /// write object to response body as extension name of file
    #[cfg(feature = "file")]
    #[cfg_attr(feature = "docs", doc(cfg(feature = "file")))]
    async fn write_file<P>(&mut self, path: P, typ: DispositionType) -> Result
    where
        P: Send + AsRef<Path>;
}

#[async_trait]
impl<S: State> PowerBody for Context<S> {
    #[inline]
    async fn read(&mut self) -> Result<Vec<u8>> {
        let mut data = match self.req.headers.typed_get::<ContentLength>() {
            Some(hint) => Vec::with_capacity(hint.0 as usize),
            None => Vec::new(),
        };
        self.req.reader().read_to_end(&mut data).await?;
        Ok(data)
    }

    #[cfg(feature = "json")]
    #[inline]
    async fn read_json<B>(&mut self) -> Result<B>
    where
        B: DeserializeOwned,
    {
        use http::StatusCode;

        use crate::status;
        let data = self.read().await?;
        serde_json::from_slice(&data).map_err(|err| status!(StatusCode::BAD_REQUEST, err))
    }

    #[cfg(feature = "urlencoded")]
    #[inline]
    async fn read_form<B>(&mut self) -> Result<B>
    where
        B: DeserializeOwned,
    {
        use http::StatusCode;

        use crate::status;
        let data = self.read().await?;
        serde_urlencoded::from_bytes(&data).map_err(|err| status!(StatusCode::BAD_REQUEST, err))
    }

    #[cfg(feature = "multipart")]
    async fn read_multipart(&mut self) -> Result<Multipart> {
        use headers::{ContentType, HeaderMapExt};
        // Verify that the request is 'Content-Type: multipart/*'.
        let typ: mime::Mime = self
            .req
            .headers
            .typed_get::<ContentType>()
            .ok_or_else(|| {
                crate::status!(http::StatusCode::BAD_REQUEST, "fail to get content-type")
            })?
            .into();
        let boundary = typ
            .get_param(mime::BOUNDARY)
            .ok_or_else(|| crate::status!(http::StatusCode::BAD_REQUEST, "fail to get boundary"))?
            .as_str();
        Ok(Multipart::new(self.req.stream(), boundary))
    }

    #[cfg(feature = "json")]
    #[inline]
    fn write_json<B>(&mut self, data: &B) -> Result
    where
        B: Serialize,
    {
        self.resp.write(serde_json::to_vec(data)?);
        self.resp.headers.typed_insert(ContentType::json());
        Ok(())
    }

    #[cfg(feature = "template")]
    #[inline]
    fn render<B>(&mut self, data: &B) -> Result
    where
        B: Template,
    {
        self.resp.write(data.render()?);
        self.resp
            .headers
            .typed_insert::<ContentType>(mime::TEXT_HTML_UTF_8.into());
        Ok(())
    }

    #[inline]
    fn write<B>(&mut self, data: B)
    where
        B: Into<Bytes>,
    {
        self.resp.write(data);
        self.resp.headers.typed_insert(ContentType::text());
    }

    #[inline]
    fn write_reader<B>(&mut self, reader: B)
    where
        B: 'static + AsyncRead + Unpin + Sync + Send,
    {
        self.resp.write_reader(reader);
        self.resp.headers.typed_insert(ContentType::octet_stream());
    }

    #[cfg(feature = "file")]
    #[inline]
    async fn write_file<P>(&mut self, path: P, typ: DispositionType) -> Result
    where
        P: Send + AsRef<Path>,
    {
        write_file(self, path, typ).await
    }
}

#[cfg(all(test, feature = "tcp"))]
mod tests {
    use std::error::Error;

    use askama::Template;
    use async_std::fs::File;
    use async_std::task::spawn;
    use http::header::CONTENT_TYPE;
    use http::StatusCode;
    use serde::{Deserialize, Serialize};

    use super::PowerBody;
    use crate::tcp::Listener;
    use crate::{http, App, Context};

    #[derive(Debug, Deserialize)]
    struct UserDto {
        id: u64,
        name: String,
    }

    #[derive(Debug, Serialize, Hash, Eq, PartialEq, Clone, Template)]
    #[template(path = "user.html")]
    struct User<'a> {
        id: u64,
        name: &'a str,
    }

    impl PartialEq<UserDto> for User<'_> {
        fn eq(&self, other: &UserDto) -> bool {
            self.id == other.id && self.name == other.name
        }
    }

    #[allow(dead_code)]
    const USER: User = User {
        id: 0,
        name: "Hexilee",
    };

    #[cfg(feature = "json")]
    #[tokio::test]
    async fn read_json() -> Result<(), Box<dyn Error>> {
        async fn test(ctx: &mut Context) -> crate::Result {
            let user: UserDto = ctx.read_json().await?;
            assert_eq!(USER, user);
            Ok(())
        }
        let (addr, server) = App::new().end(test).run()?;
        spawn(server);

        let client = reqwest::Client::new();
        let resp = client
            .get(&format!("http://{}", addr))
            .json(&USER)
            .send()
            .await?;
        assert_eq!(StatusCode::OK, resp.status());
        Ok(())
    }

    #[cfg(feature = "urlencoded")]
    #[tokio::test]
    async fn read_form() -> Result<(), Box<dyn Error>> {
        async fn test(ctx: &mut Context) -> crate::Result {
            let user: UserDto = ctx.read_form().await?;
            assert_eq!(USER, user);
            Ok(())
        }
        let (addr, server) = App::new().end(test).run()?;
        spawn(server);

        let client = reqwest::Client::new();
        let resp = client
            .get(&format!("http://{}", addr))
            .form(&USER)
            .send()
            .await?;
        assert_eq!(StatusCode::OK, resp.status());
        Ok(())
    }

    #[cfg(feature = "template")]
    #[tokio::test]
    async fn render() -> Result<(), Box<dyn Error>> {
        async fn test(ctx: &mut Context) -> crate::Result {
            ctx.render(&USER)
        }
        let (addr, server) = App::new().end(test).run()?;
        spawn(server);
        let resp = reqwest::get(&format!("http://{}", addr)).await?;
        assert_eq!(StatusCode::OK, resp.status());
        assert_eq!("text/html; charset=utf-8", resp.headers()[CONTENT_TYPE]);
        Ok(())
    }

    #[tokio::test]
    async fn write() -> Result<(), Box<dyn Error>> {
        async fn test(ctx: &mut Context) -> crate::Result {
            ctx.write("Hello, World!");
            Ok(())
        }
        let (addr, server) = App::new().end(test).run()?;
        spawn(server);
        let resp = reqwest::get(&format!("http://{}", addr)).await?;
        assert_eq!(StatusCode::OK, resp.status());
        assert_eq!("text/plain", resp.headers()[CONTENT_TYPE]);
        assert_eq!("Hello, World!", resp.text().await?);
        Ok(())
    }

    #[tokio::test]
    async fn write_octet() -> Result<(), Box<dyn Error>> {
        async fn test(ctx: &mut Context) -> crate::Result {
            ctx.write_reader(File::open("../assets/author.txt").await?);
            Ok(())
        }
        let (addr, server) = App::new().end(test).run()?;
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

    #[cfg(feature = "multipart")]
    mod multipart {
        use std::error::Error as StdError;

        use async_std::fs::read;
        use reqwest::multipart::{Form, Part};
        use reqwest::Client;

        use crate::body::PowerBody;
        use crate::http::header::CONTENT_TYPE;
        use crate::http::StatusCode;
        use crate::router::{post, Router};
        use crate::tcp::Listener;
        use crate::{throw, App, Context};

        const FILE_PATH: &str = "../assets/author.txt";
        const FILE_NAME: &str = "author.txt";
        const FIELD_NAME: &str = "file";

        async fn post_file(ctx: &mut Context) -> crate::Result {
            let mut form = ctx.read_multipart().await?;
            while let Some(field) = form.next_field().await? {
                match (field.file_name(), field.name()) {
                    (Some(filename), Some(name)) => {
                        assert_eq!(FIELD_NAME, name);
                        assert_eq!(FILE_NAME, filename);
                        let content = field.bytes().await?;
                        let expected_content = read(FILE_PATH).await?;
                        assert_eq!(&expected_content, &content);
                    }
                    _ => throw!(
                        StatusCode::BAD_REQUEST,
                        format!("invalid field: {:?}", field)
                    ),
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
                .multipart(form)
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
}
