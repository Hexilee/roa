mod decode;
mod json;
mod mime_ext;
mod urlencoded;

use crate::header::StringHeaders;
use crate::{throw, Context, Model, Status};
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

#[async_trait]
pub trait PowerBody {
    /// try to get mime content type of request.
    async fn request_type(&self) -> Option<Result<Mime, Status>>;

    /// try to get mime content type of response.
    async fn response_type(&self) -> Option<Result<Mime, Status>>;

    /// read request body as Vec<u8>.
    async fn body_buf(&self) -> Result<Vec<u8>, Status>;

    /// read request body by Content-Type.
    async fn read<B: DeserializeOwned>(&self) -> Result<B, Status>;

    /// read request body as "application/json".
    async fn read_json<B: DeserializeOwned>(&self) -> Result<B, Status>;

    /// read request body as "application/x-www-form-urlencoded".
    async fn read_form<B: DeserializeOwned>(&self) -> Result<B, Status>;

    // read request body as "multipart/form-data"
    // async fn read_multipart(&self) -> Result<B, Status>;

    /// write object to response body as "application/json; charset=utf-8"
    async fn write_json<B: Serialize + Sync>(&self, data: &B) -> Result<(), Status>;

    /// write object to response body as "text/html; charset=utf-8"
    async fn render<B: Template + Sync>(&self, data: &B) -> Result<(), Status>;

    /// write object to response body as "text/plain; charset=utf-8"
    async fn write_text<S: ToString + Send>(&self, string: S) -> Result<(), Status>;

    /// write object to response body as "application/octet-stream"
    async fn write_octet<B: 'static + BufRead + Unpin + Sync + Send>(
        &self,
        reader: B,
    ) -> Result<(), Status>;

    /// write object to response body as extension name of file
    async fn write_file<P: AsRef<Path> + Send>(&self, path: P) -> Result<(), Status>;
}

fn parse_mime(value: &str) -> Result<Mime, Status> {
    value.parse().map_err(|err| {
        Status::new(
            StatusCode::BAD_REQUEST,
            format!("{}\nContent-Type value is invalid", err),
            true,
        )
    })
}

#[async_trait]
impl<M: Model> PowerBody for Context<M> {
    async fn request_type(&self) -> Option<Result<Mime, Status>> {
        self.req()
            .await
            .get(http::header::CONTENT_TYPE)
            .map(|result| result.and_then(parse_mime))
    }

    async fn response_type(&self) -> Option<Result<Mime, Status>> {
        self.resp()
            .await
            .get(http::header::CONTENT_TYPE)
            .map(|result| result.and_then(parse_mime))
    }

    async fn body_buf(&self) -> Result<Vec<u8>, Status> {
        let mut data = Vec::new();
        self.req_mut().await.read_to_end(&mut data).await?;
        Ok(data)
    }

    // return BAD_REQUEST status while parsing Content-Type fails.
    // Content-Type can only be JSON or URLENCODED, otherwise this function will return UNSUPPORTED_MEDIA_TYPE error.
    async fn read<B: DeserializeOwned>(&self) -> Result<B, Status> {
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

    async fn read_json<B: DeserializeOwned>(&self) -> Result<B, Status> {
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

    async fn read_form<B: DeserializeOwned>(&self) -> Result<B, Status> {
        urlencoded::from_bytes(&self.body_buf().await?)
    }

    async fn write_json<B: Serialize + Sync>(&self, data: &B) -> Result<(), Status> {
        self.resp_mut().await.write_bytes(json::to_bytes(data)?);
        self.resp_mut().await.insert(
            http::header::CONTENT_TYPE,
            "application/json; charset=utf-8",
        )?;
        Ok(())
    }

    async fn render<B: Template + Sync>(&self, data: &B) -> Result<(), Status> {
        self.resp_mut().await.write_str(
            data.render()
                .map_err(|err| Status::new(StatusCode::INTERNAL_SERVER_ERROR, err, false))?,
        );
        self.resp_mut()
            .await
            .insert(http::header::CONTENT_TYPE, &mime::TEXT_HTML_UTF_8)?;
        Ok(())
    }

    async fn write_text<S: ToString + Send>(&self, string: S) -> Result<(), Status> {
        self.resp_mut().await.write_str(string.to_string());
        self.resp_mut()
            .await
            .insert(http::header::CONTENT_TYPE, &mime::TEXT_PLAIN_UTF_8)?;
        Ok(())
    }

    async fn write_octet<B: 'static + BufRead + Unpin + Sync + Send>(
        &self,
        reader: B,
    ) -> Result<(), Status> {
        self.resp_mut().await.write_buf(reader);
        self.resp_mut()
            .await
            .insert(http::header::CONTENT_TYPE, &mime::APPLICATION_OCTET_STREAM)?;
        Ok(())
    }

    async fn write_file<P: AsRef<Path> + Send>(&self, path: P) -> Result<(), Status> {
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
