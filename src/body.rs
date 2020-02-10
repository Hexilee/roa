mod decode;
mod json;
mod mime_ext;
mod urlencoded;

use crate::core::{throw, Context, Error, Model, Result};
use crate::header::FriendlyHeaders;
use askama::Template;
use async_std::fs::File;
use async_trait::async_trait;
use futures::{AsyncBufRead as BufRead, AsyncReadExt};
use http::StatusCode;
use mime::Mime;
use mime_ext::MimeExt;
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::path::Path;

const APPLICATION_JSON_UTF_8: &str = "application/json; charset=utf-8";

/// A ContextExt.
#[async_trait]
pub trait PowerBody {
    /// try to get mime content type of request.
    async fn request_type(&self) -> Option<Result<Mime>>;

    /// try to get mime content type of response.
    async fn response_type(&self) -> Option<Result<Mime>>;

    /// read request body as Vec<u8>.
    async fn body_buf(&self) -> Result<Vec<u8>>;

    /// read request body by Content-Type.
    async fn read<B: DeserializeOwned>(&self) -> Result<B>;

    /// read request body as "application/json".
    async fn read_json<B: DeserializeOwned>(&self) -> Result<B>;

    /// read request body as "application/x-www-form-urlencoded".
    async fn read_form<B: DeserializeOwned>(&self) -> Result<B>;

    // read request body as "multipart/form-data"
    // async fn read_multipart(&self) -> Result<B, Status>;

    /// write object to response body as "application/json; charset=utf-8"
    async fn write_json<B: Serialize + Sync>(&self, data: &B) -> Result;

    /// write object to response body as "text/html; charset=utf-8"
    async fn render<B: Template + Sync>(&self, data: &B) -> Result;

    /// write object to response body as "text/plain; charset=utf-8"
    async fn write_text<S: ToString + Send>(&self, string: S) -> Result;

    /// write object to response body as "application/octet-stream"
    async fn write_octet<B: 'static + BufRead + Unpin + Sync + Send>(&self, reader: B) -> Result;

    /// write object to response body as extension name of file
    async fn write_file<P: AsRef<Path> + Send>(&self, path: P) -> Result;
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
impl<M: Model> PowerBody for Context<M> {
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

    async fn body_buf(&self) -> Result<Vec<u8>> {
        let mut data = Vec::new();
        self.req_mut().await.read_to_end(&mut data).await?;
        Ok(data)
    }

    // return BAD_REQUEST status while parsing Content-Type fails.
    // Content-Type can only be JSON or URLENCODED, otherwise this function will return UNSUPPORTED_MEDIA_TYPE error.
    async fn read<B: DeserializeOwned>(&self) -> Result<B> {
        match self.request_type().await {
            None => self.read_json().await,
            Some(ret) => {
                let mime_type = ret?.pure_type();
                if mime_type == mime::APPLICATION_JSON {
                    self.read_json().await
                } else if mime_type == mime::APPLICATION_WWW_FORM_URLENCODED {
                    self.read_form().await
                } else {
                    throw(
                        StatusCode::UNSUPPORTED_MEDIA_TYPE,
                        "Content-Type can only be JSON or URLENCODED",
                    )
                }
            }
        }
    }

    async fn read_json<B: DeserializeOwned>(&self) -> Result<B> {
        let data = self.body_buf().await?;
        match self.request_type().await {
            None | Some(Err(_)) => json::from_bytes(&data),
            Some(Ok(mime_type)) => {
                if mime_type.pure_type() != mime::APPLICATION_JSON {
                    json::from_bytes(&data)
                } else {
                    match mime_type.get_param("charset") {
                        None | Some(mime::UTF_8) => json::from_bytes(&data),
                        Some(charset) => json::from_str(&decode::decode(&data, charset.as_str())?),
                    }
                }
            }
        }
    }

    async fn read_form<B: DeserializeOwned>(&self) -> Result<B> {
        urlencoded::from_bytes(&self.body_buf().await?)
    }

    async fn write_json<B: Serialize + Sync>(&self, data: &B) -> Result {
        self.resp_mut().await.write_bytes(json::to_bytes(data)?);
        self.resp_mut()
            .await
            .insert(http::header::CONTENT_TYPE, APPLICATION_JSON_UTF_8)?;
        Ok(())
    }

    async fn render<B: Template + Sync>(&self, data: &B) -> Result {
        self.resp_mut().await.write_str(
            data.render()
                .map_err(|err| Error::new(StatusCode::INTERNAL_SERVER_ERROR, err, false))?,
        );
        self.resp_mut()
            .await
            .insert(http::header::CONTENT_TYPE, &mime::TEXT_HTML_UTF_8)?;
        Ok(())
    }

    async fn write_text<S: ToString + Send>(&self, string: S) -> Result {
        self.resp_mut().await.write_str(string.to_string());
        self.resp_mut()
            .await
            .insert(http::header::CONTENT_TYPE, &mime::TEXT_PLAIN_UTF_8)?;
        Ok(())
    }

    async fn write_octet<B: 'static + BufRead + Unpin + Sync + Send>(&self, reader: B) -> Result {
        self.resp_mut().await.write_buf(reader);
        self.resp_mut()
            .await
            .insert(http::header::CONTENT_TYPE, &mime::APPLICATION_OCTET_STREAM)?;
        Ok(())
    }

    async fn write_file<P: AsRef<Path> + Send>(&self, path: P) -> Result {
        let path = path.as_ref();
        self.resp_mut().await.write(File::open(path).await?);

        if let Some(filename) = path.file_name() {
            self.resp_mut().await.insert(
                http::header::CONTENT_TYPE,
                &mime_guess::from_path(&filename).first_or_octet_stream(),
            )?;
            let encoded_filename =
                utf8_percent_encode(&filename.to_string_lossy(), NON_ALPHANUMERIC).to_string();
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
            .gate_fn(move |ctx, _next| async move {
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
            .gate_fn(move |ctx, _next| async move {
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
            .gate_fn(move |ctx, _next| async move { ctx.write_text("Hello, World!").await })
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
            .gate_fn(move |ctx, _next| async move {
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
            .gate_fn(move |ctx, _next| async move {
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
